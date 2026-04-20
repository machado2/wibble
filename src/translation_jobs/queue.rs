use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::entities::{prelude::*, translation_job};
use crate::error::Error;
use crate::llm::prompt_registry::SupportedTranslationLanguage;
use crate::rate_limit::{RequesterTier, TranslationRateLimit};
use crate::services::article_language::article_source_language;

use super::definitions::{
    article_translation_job_id, request_priority, TranslationJobRequestSource,
    TRANSLATION_JOB_STATUS_FAILED, TRANSLATION_JOB_STATUS_PROCESSING,
    TRANSLATION_JOB_STATUS_QUEUED,
};
use super::support::{now, should_requeue_job, translation_resume_batch_size};

pub(super) async fn load_translation_job(
    db: &DatabaseConnection,
    job_id: &str,
) -> Result<Option<translation_job::Model>, Error> {
    TranslationJob::find_by_id(job_id.to_string())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading translation job {}: {}", job_id, e)))
}

pub(super) async fn persist_translation_job_request(
    state: &AppState,
    article_id: &str,
    language: SupportedTranslationLanguage,
    request_source: TranslationJobRequestSource,
    requester_tier: RequesterTier,
    rate_limit_key: &str,
    enforce_rate_limit: bool,
) -> Result<(), Error> {
    let job_id = article_translation_job_id(article_id, language);
    let reference_time = now();
    let existing = load_translation_job(&state.db, &job_id).await?;
    let needs_requeue = existing
        .as_ref()
        .is_none_or(|job| should_requeue_job(job, reference_time));

    if needs_requeue && enforce_rate_limit {
        state
            .rate_limit_state
            .check_translation_generation_limit(requester_tier, rate_limit_key)
            .map_err(|limit| {
                let limit_name = match limit {
                    TranslationRateLimit::Hourly => "hourly",
                    TranslationRateLimit::Daily => "daily",
                };
                event!(
                    Level::WARN,
                    article_id,
                    language = language.code,
                    limit = limit_name,
                    tier = ?requester_tier,
                    "Rejected translation creation due to translation rate limit"
                );
                Error::RateLimited
            })?;
    }

    match existing {
        Some(existing) => {
            let previous_status = existing.status.clone();
            let requested_priority = request_priority(request_source, requester_tier);
            let merged_priority = existing.priority.max(requested_priority);
            let merged_request_source = if requested_priority >= existing.priority {
                request_source.as_str().to_string()
            } else {
                existing.request_source.clone()
            };
            let mut active: translation_job::ActiveModel = existing.into();
            active.request_source = ActiveValue::set(merged_request_source);
            active.priority = ActiveValue::set(merged_priority);
            active.updated_at = ActiveValue::set(reference_time);
            if needs_requeue {
                active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_QUEUED.to_string());
                active.last_error = ActiveValue::set(None);
                active.started_at = ActiveValue::set(None);
                active.finished_at = ActiveValue::set(None);
                active.next_retry_at = ActiveValue::set(None);
                if previous_status != TRANSLATION_JOB_STATUS_FAILED {
                    active.fail_count = ActiveValue::set(0);
                }
            }
            active
                .update(&state.db)
                .await
                .map_err(|e| Error::Database(format!("Error updating translation job: {}", e)))?;
        }
        None => {
            TranslationJob::insert(translation_job::ActiveModel {
                id: ActiveValue::set(job_id),
                article_id: ActiveValue::set(article_id.to_string()),
                language_code: ActiveValue::set(language.code.to_string()),
                request_source: ActiveValue::set(request_source.as_str().to_string()),
                priority: ActiveValue::set(request_priority(request_source, requester_tier)),
                status: ActiveValue::set(TRANSLATION_JOB_STATUS_QUEUED.to_string()),
                fail_count: ActiveValue::set(0),
                last_error: ActiveValue::set(None),
                created_at: ActiveValue::set(reference_time),
                updated_at: ActiveValue::set(reference_time),
                started_at: ActiveValue::set(None),
                finished_at: ActiveValue::set(None),
                next_retry_at: ActiveValue::set(None),
            })
            .exec(&state.db)
            .await
            .map_err(|e| Error::Database(format!("Error inserting translation job: {}", e)))?;
        }
    }

    Ok(())
}

pub(super) async fn due_translation_jobs(
    state: &AppState,
) -> Result<Vec<translation_job::Model>, Error> {
    let reference_time = now();

    TranslationJob::find()
        .filter(
            Condition::any()
                .add(translation_job::Column::Status.is_in([
                    TRANSLATION_JOB_STATUS_QUEUED,
                    TRANSLATION_JOB_STATUS_PROCESSING,
                ]))
                .add(
                    Condition::all()
                        .add(translation_job::Column::Status.eq(TRANSLATION_JOB_STATUS_FAILED))
                        .add(
                            Condition::any()
                                .add(translation_job::Column::NextRetryAt.is_null())
                                .add(translation_job::Column::NextRetryAt.lte(reference_time)),
                        ),
                ),
        )
        .order_by_desc(translation_job::Column::Priority)
        .order_by_asc(translation_job::Column::CreatedAt)
        .limit(translation_resume_batch_size())
        .all(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading translation jobs: {}", e)))
}

pub(super) async fn delete_translation_job(state: &AppState, job_id: &str) -> Result<(), Error> {
    TranslationJob::delete_by_id(job_id.to_string())
        .exec(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!("Error deleting translation job {}: {}", job_id, e))
        })?;
    Ok(())
}

pub(super) async fn queue_translation_refresh(
    state: &AppState,
    article_id: &str,
    language: SupportedTranslationLanguage,
    requester_tier: RequesterTier,
    rate_limit_key: &str,
) -> Result<(), Error> {
    persist_translation_job_request(
        state,
        article_id,
        language,
        TranslationJobRequestSource::EditRefresh,
        requester_tier,
        rate_limit_key,
        false,
    )
    .await
}

pub(super) fn stale_translation_languages_for_refresh(
    cached_languages: &[SupportedTranslationLanguage],
) -> Vec<SupportedTranslationLanguage> {
    cached_languages
        .iter()
        .copied()
        .filter(|language| language.code != article_source_language().code)
        .collect()
}
