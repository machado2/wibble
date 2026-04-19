use std::env;
use std::time::Duration;

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, DatabaseConnection, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect,
};
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::article_id::normalize_content_model;
use crate::audit::{log_audit, log_system_audit};
use crate::auth::AuthUser;
use crate::entities::{prelude::*, translation_job};
use crate::error::Error;
use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};
use crate::llm::translate::Translate;
use crate::rate_limit::{RequesterTier, TranslationRateLimit};
use crate::services::article_language::{article_source_language, PreferredLanguageSource};
use crate::services::article_translations::{
    article_translation_job_key, cached_translation_languages, ensure_cached_article_translation,
    invalidate_cached_article_translations, load_cached_article_translation,
    owned_article_source_text, OwnedArticleSourceText,
};

pub const TRANSLATION_JOB_STATUS_QUEUED: &str = "queued";
pub const TRANSLATION_JOB_STATUS_PROCESSING: &str = "processing";
pub const TRANSLATION_JOB_STATUS_COMPLETED: &str = "completed";
pub const TRANSLATION_JOB_STATUS_FAILED: &str = "failed";
pub const TRANSLATION_JOB_STATUS_CANCELLED: &str = "cancelled";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationJobRequestSource {
    Explicit,
    Cookie,
    Browser,
    EditRefresh,
}

impl TranslationJobRequestSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Cookie => "cookie",
            Self::Browser => "browser",
            Self::EditRefresh => "edit_refresh",
        }
    }

    fn priority(self) -> i32 {
        match self {
            Self::Explicit => 30,
            Self::EditRefresh => 25,
            Self::Cookie => 20,
            Self::Browser => 10,
        }
    }
}

pub fn request_source_from_preferred_language(
    source: PreferredLanguageSource,
) -> TranslationJobRequestSource {
    match source {
        PreferredLanguageSource::Explicit => TranslationJobRequestSource::Explicit,
        PreferredLanguageSource::Cookie => TranslationJobRequestSource::Cookie,
        PreferredLanguageSource::Browser | PreferredLanguageSource::ArticleSource => {
            TranslationJobRequestSource::Browser
        }
    }
}

fn translation_resume_interval_seconds() -> u64 {
    env::var("TRANSLATION_RESUME_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(30)
}

fn translation_resume_batch_size() -> u64 {
    env::var("TRANSLATION_RESUME_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50)
}

fn translation_retry_base_seconds() -> u64 {
    env::var("TRANSLATION_RETRY_BASE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(60)
}

fn translation_retry_max_seconds() -> u64 {
    env::var("TRANSLATION_RETRY_MAX_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(15 * 60)
}

fn translation_retry_delay(fail_count: i32) -> Duration {
    let exponent = fail_count.saturating_sub(1).clamp(0, 4) as u32;
    let retry_seconds = translation_retry_base_seconds()
        .saturating_mul(1_u64 << exponent)
        .min(translation_retry_max_seconds());
    Duration::from_secs(retry_seconds)
}

fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_local()
}

fn should_requeue_job(job: &translation_job::Model, reference_time: chrono::NaiveDateTime) -> bool {
    match job.status.as_str() {
        TRANSLATION_JOB_STATUS_QUEUED | TRANSLATION_JOB_STATUS_PROCESSING => false,
        TRANSLATION_JOB_STATUS_FAILED => job
            .next_retry_at
            .is_none_or(|retry_at| retry_at <= reference_time),
        TRANSLATION_JOB_STATUS_COMPLETED => true,
        _ => true,
    }
}

async fn load_translation_job(
    db: &DatabaseConnection,
    job_id: &str,
) -> Result<Option<translation_job::Model>, Error> {
    TranslationJob::find_by_id(job_id.to_string())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading translation job {}: {}", job_id, e)))
}

async fn persist_translation_job_request(
    state: &AppState,
    article_id: &str,
    language: SupportedTranslationLanguage,
    request_source: TranslationJobRequestSource,
    requester_tier: RequesterTier,
    rate_limit_key: &str,
    enforce_rate_limit: bool,
) -> Result<(), Error> {
    let job_id = article_translation_job_key(article_id, language);
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
            let requested_priority =
                request_source.priority() + requester_tier.queue_priority_boost();
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
                priority: ActiveValue::set(
                    request_source.priority() + requester_tier.queue_priority_boost(),
                ),
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

async fn due_translation_jobs(state: &AppState) -> Result<Vec<translation_job::Model>, Error> {
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

async fn mark_processing(
    state: &AppState,
    job: translation_job::Model,
) -> Result<translation_job::Model, Error> {
    let Some(current) = load_translation_job(&state.db, &job.id).await? else {
        return Ok(job);
    };
    if !matches!(
        current.status.as_str(),
        TRANSLATION_JOB_STATUS_QUEUED
            | TRANSLATION_JOB_STATUS_FAILED
            | TRANSLATION_JOB_STATUS_PROCESSING
    ) {
        return Ok(current);
    }
    let mut active: translation_job::ActiveModel = current.into();
    let reference_time = now();
    active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_PROCESSING.to_string());
    active.last_error = ActiveValue::set(None);
    active.started_at = ActiveValue::set(Some(reference_time));
    active.finished_at = ActiveValue::set(None);
    active.next_retry_at = ActiveValue::set(None);
    active.updated_at = ActiveValue::set(reference_time);
    active.update(&state.db).await.map_err(|e| {
        Error::Database(format!(
            "Error marking translation job as processing: {}",
            e
        ))
    })
}

async fn mark_completed(
    state: &AppState,
    job: translation_job::Model,
    outcome: &str,
) -> Result<(), Error> {
    let Some(current) = load_translation_job(&state.db, &job.id).await? else {
        return Ok(());
    };
    if current.status != TRANSLATION_JOB_STATUS_PROCESSING {
        return Ok(());
    }
    let mut active: translation_job::ActiveModel = current.into();
    let reference_time = now();
    active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_COMPLETED.to_string());
    active.fail_count = ActiveValue::set(0);
    active.last_error = ActiveValue::set(None);
    active.finished_at = ActiveValue::set(Some(reference_time));
    active.next_retry_at = ActiveValue::set(None);
    active.updated_at = ActiveValue::set(reference_time);
    active.update(&state.db).await.map_err(|e| {
        Error::Database(format!("Error marking translation job as completed: {}", e))
    })?;

    let details = serde_json::json!({
        "article_id": job.article_id,
        "language": job.language_code,
        "request_source": job.request_source,
        "outcome": outcome,
    })
    .to_string();
    log_system_audit(
        &state.db,
        "translation_job_completed",
        "translation_job",
        &job.id,
        Some(details),
    )
    .await?;
    Ok(())
}

async fn mark_failed(
    state: &AppState,
    job: translation_job::Model,
    err: &Error,
) -> Result<(), Error> {
    let Some(current) = load_translation_job(&state.db, &job.id).await? else {
        return Ok(());
    };
    if current.status != TRANSLATION_JOB_STATUS_PROCESSING {
        return Ok(());
    }
    let mut active: translation_job::ActiveModel = current.into();
    let reference_time = now();
    let fail_count = job.fail_count + 1;
    let retry_at = reference_time
        + chrono::Duration::from_std(translation_retry_delay(fail_count))
            .unwrap_or_else(|_| chrono::Duration::seconds(translation_retry_max_seconds() as i64));
    active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_FAILED.to_string());
    active.fail_count = ActiveValue::set(fail_count);
    active.last_error = ActiveValue::set(Some(err.to_string()));
    active.finished_at = ActiveValue::set(Some(reference_time));
    active.next_retry_at = ActiveValue::set(Some(retry_at));
    active.updated_at = ActiveValue::set(reference_time);
    active
        .update(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error marking translation job as failed: {}", e)))?;

    let details = serde_json::json!({
        "article_id": job.article_id,
        "language": job.language_code,
        "request_source": job.request_source,
        "fail_count": fail_count,
        "retry_at": retry_at,
        "error": err.to_string(),
    })
    .to_string();
    log_system_audit(
        &state.db,
        "translation_job_failed",
        "translation_job",
        &job.id,
        Some(details),
    )
    .await?;
    Ok(())
}

pub async fn cancel_translation_job(state: &AppState, job_id: &str) -> Result<bool, Error> {
    let Some(job) = load_translation_job(&state.db, job_id).await? else {
        return Ok(false);
    };
    if job.status == TRANSLATION_JOB_STATUS_CANCELLED {
        state.mark_translation_generation_finished(job_id).await;
        return Ok(true);
    }
    if matches!(
        job.status.as_str(),
        TRANSLATION_JOB_STATUS_COMPLETED | TRANSLATION_JOB_STATUS_CANCELLED
    ) {
        return Err(Error::BadRequest(format!(
            "Translation job {} is already {}",
            job_id, job.status
        )));
    }

    let mut active: translation_job::ActiveModel = job.into();
    let reference_time = now();
    active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_CANCELLED.to_string());
    active.last_error = ActiveValue::set(Some("Cancelled by admin".to_string()));
    active.finished_at = ActiveValue::set(Some(reference_time));
    active.next_retry_at = ActiveValue::set(None);
    active.updated_at = ActiveValue::set(reference_time);
    active
        .update(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error cancelling translation job: {}", e)))?;
    state.mark_translation_generation_finished(job_id).await;
    Ok(true)
}

async fn delete_translation_job(state: &AppState, job_id: &str) -> Result<(), Error> {
    TranslationJob::delete_by_id(job_id.to_string())
        .exec(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!("Error deleting translation job {}: {}", job_id, e))
        })?;
    Ok(())
}

async fn process_translation_job_with_translator<T: Translate>(
    state: &AppState,
    translator: &T,
    job_id: &str,
) -> Result<(), Error> {
    let Some(job) = load_translation_job(&state.db, job_id).await? else {
        return Ok(());
    };
    let Some(language) = find_supported_translation_language(&job.language_code) else {
        let details = serde_json::json!({
            "language": job.language_code,
            "reason": "unsupported_language",
        })
        .to_string();
        log_system_audit(
            &state.db,
            "translation_job_dropped",
            "translation_job",
            &job.id,
            Some(details),
        )
        .await?;
        return delete_translation_job(state, &job.id).await;
    };
    let job = mark_processing(state, job).await?;
    if job.status != TRANSLATION_JOB_STATUS_PROCESSING {
        return Ok(());
    }
    let Some(article) = Content::find_by_id(job.article_id.clone())
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article for translation: {}", e)))?
        .map(normalize_content_model)
    else {
        return Ok(());
    };
    if article.generating {
        let err = Error::NotFound(Some(format!(
            "Article {} is still generating; translation deferred",
            article.id
        )));
        mark_failed(state, job, &err).await?;
        return Err(err);
    }

    let Some(source) = owned_article_source_text(&article) else {
        let err = Error::NotFound(Some(format!(
            "Article {} has no markdown available for translation",
            article.id
        )));
        mark_failed(state, job, &err).await?;
        return Err(err);
    };
    let source_ref = source.as_ref();
    if load_cached_article_translation(&state.db, source_ref, language)
        .await?
        .is_some()
    {
        mark_completed(state, job, "cached").await?;
        return Ok(());
    }

    ensure_cached_article_translation(translator, &state.db, source_ref, language).await?;
    mark_completed(state, job, "generated").await?;
    Ok(())
}

async fn process_translation_job(state: &AppState, job_id: &str) -> Result<(), Error> {
    process_translation_job_with_translator(state, &state.llm, job_id).await
}

fn spawn_translation_job(
    state: AppState,
    permit: tokio::sync::OwnedSemaphorePermit,
    job_id: String,
) {
    tokio::spawn(async move {
        let result = process_translation_job(&state, &job_id).await;
        if let Err(err) = result {
            event!(
                Level::ERROR,
                job_id = %job_id,
                error = %err,
                "Translation job failed"
            );
        } else {
            event!(Level::INFO, job_id = %job_id, "Translation job finished");
        }
        drop(permit);
        state.mark_translation_generation_finished(&job_id).await;
    });
}

pub async fn spawn_due_translation_jobs(state: AppState) {
    let jobs = match due_translation_jobs(&state).await {
        Ok(jobs) => jobs,
        Err(err) => {
            event!(Level::ERROR, error = %err, "Failed to load due translation jobs");
            return;
        }
    };

    for job in jobs {
        let permit = match state
            .translation_generation_semaphore
            .clone()
            .try_acquire_owned()
        {
            Ok(permit) => permit,
            Err(_) => break,
        };
        if !state.try_mark_translation_generation_started(&job.id).await {
            continue;
        }
        spawn_translation_job(state.clone(), permit, job.id);
    }
}

pub async fn request_article_translation(
    state: AppState,
    article_id: String,
    language: SupportedTranslationLanguage,
    request_source: TranslationJobRequestSource,
    requester_tier: RequesterTier,
    rate_limit_key: String,
) {
    if let Err(err) = persist_translation_job_request(
        &state,
        &article_id,
        language,
        request_source,
        requester_tier,
        &rate_limit_key,
        true,
    )
    .await
    {
        event!(
            Level::WARN,
            article_id,
            language = language.code,
            error = %err,
            "Failed to queue article translation"
        );
        return;
    }
    spawn_due_translation_jobs(state).await;
}

async fn queue_translation_refresh(
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

fn stale_translation_languages_for_refresh(
    cached_languages: &[SupportedTranslationLanguage],
) -> Vec<SupportedTranslationLanguage> {
    cached_languages
        .iter()
        .copied()
        .filter(|language| language.code != article_source_language().code)
        .collect()
}

pub async fn refresh_article_translations_after_edit(
    state: AppState,
    auth_user: &AuthUser,
    slug: &str,
    previous_source: OwnedArticleSourceText,
    current_source: OwnedArticleSourceText,
) -> Result<(), Error> {
    if previous_source == current_source {
        return Ok(());
    }

    let cached_languages =
        cached_translation_languages(&state.db, previous_source.as_ref()).await?;
    if cached_languages.is_empty() {
        return Ok(());
    }

    let stale_languages = stale_translation_languages_for_refresh(&cached_languages);
    if stale_languages.is_empty() {
        return Ok(());
    }

    let removed_rows =
        invalidate_cached_article_translations(&state.db, previous_source.as_ref()).await?;
    let details = serde_json::json!({
        "languages": stale_languages
            .iter()
            .map(|language| language.code)
            .collect::<Vec<_>>(),
        "removed_rows": removed_rows,
    })
    .to_string();
    log_audit(
        &state.db,
        auth_user,
        "invalidate_article_translations",
        "content",
        slug,
        Some(details),
    )
    .await?;

    for language in stale_languages {
        let requester_tier = if auth_user.is_admin() {
            RequesterTier::Admin
        } else {
            RequesterTier::Authenticated
        };
        let rate_limit_key = format!("user:{}", auth_user.email);
        queue_translation_refresh(
            &state,
            &current_source.article_id,
            language,
            requester_tier,
            &rate_limit_key,
        )
        .await?;
    }
    spawn_due_translation_jobs(state).await;
    Ok(())
}

pub fn spawn_resume_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(translation_resume_interval_seconds()));
        loop {
            interval.tick().await;
            spawn_due_translation_jobs(state.clone()).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

    use crate::entities::{content, prelude::TranslationJob, translation_job};
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::llm::translate::Translate;
    use crate::services::article_language::PreferredLanguageSource;
    use crate::services::article_translations::{
        load_cached_article_translation, ArticleSourceText,
    };
    use crate::test_support::{preferred_language, test_state_for, TestContext};

    use super::{
        due_translation_jobs, load_translation_job, persist_translation_job_request,
        process_translation_job_with_translator, request_source_from_preferred_language,
        stale_translation_languages_for_refresh, translation_retry_delay,
        TranslationJobRequestSource, TRANSLATION_JOB_STATUS_COMPLETED,
        TRANSLATION_JOB_STATUS_PROCESSING,
    };
    use crate::error::Error;
    use crate::rate_limit::RequesterTier;

    #[derive(Default)]
    struct FakeTranslator;

    impl Translate for FakeTranslator {
        async fn translate(
            &self,
            text: &str,
            target_language: crate::llm::prompt_registry::SupportedTranslationLanguage,
        ) -> Result<String, Error> {
            Ok(format!("[{}] {}", target_language.code, text))
        }
    }

    fn sample_article(id: &str, slug: &str) -> content::ActiveModel {
        content::ActiveModel {
            id: ActiveValue::set(id.to_string()),
            slug: ActiveValue::set(slug.to_string()),
            content: ActiveValue::set(None),
            created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            generating: ActiveValue::set(false),
            generation_started_at: ActiveValue::set(None),
            generation_finished_at: ActiveValue::set(None),
            flagged: ActiveValue::set(false),
            model: ActiveValue::set("test-model".to_string()),
            prompt_version: ActiveValue::set(1),
            fail_count: ActiveValue::set(0),
            description: ActiveValue::set("Officials said the bulletin remained strictly procedural.".to_string()),
            image_id: ActiveValue::set(None),
            title: ActiveValue::set("Research Bulletin".to_string()),
            user_input: ActiveValue::set("Briefing request".to_string()),
            image_prompt: ActiveValue::set(None),
            user_email: ActiveValue::set(None),
            votes: ActiveValue::set(7),
            hot_score: ActiveValue::set(0.0),
            generation_time_ms: ActiveValue::set(None),
            flarum_id: ActiveValue::set(None),
            markdown: ActiveValue::set(Some(
                "## Committee Response\n\nThe standing committee accepted the memo without visible alarm.".to_string(),
            )),
            converted: ActiveValue::set(true),
            longview_count: ActiveValue::set(0),
            impression_count: ActiveValue::set(0),
            click_count: ActiveValue::set(0),
            author_email: ActiveValue::set(None),
            published: ActiveValue::set(true),
            recovered_from_dead_link: ActiveValue::set(false),
        }
    }

    fn article_source<'a>(article_id: &'a str) -> ArticleSourceText<'a> {
        ArticleSourceText {
            article_id,
            title: "Research Bulletin",
            description: "Officials said the bulletin remained strictly procedural.",
            markdown: "## Committee Response\n\nThe standing committee accepted the memo without visible alarm.",
        }
    }

    #[test]
    fn request_source_preserves_manual_priority_over_browser_defaults() {
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Explicit),
            TranslationJobRequestSource::Explicit
        );
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Cookie),
            TranslationJobRequestSource::Cookie
        );
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Browser),
            TranslationJobRequestSource::Browser
        );
    }

    #[test]
    fn translation_retry_delay_grows_with_failures_and_caps() {
        let first = translation_retry_delay(1);
        let second = translation_retry_delay(2);
        let sixth = translation_retry_delay(6);

        assert!(second > first);
        assert!(sixth >= second);
        assert!(sixth.as_secs() <= 15 * 60);
    }

    #[test]
    fn stale_translation_refresh_excludes_source_language() {
        let languages = vec![
            find_supported_translation_language("en").unwrap(),
            find_supported_translation_language("pt").unwrap(),
            find_supported_translation_language("fr").unwrap(),
        ];

        let stale = stale_translation_languages_for_refresh(&languages);

        assert_eq!(stale.len(), 2);
        assert!(stale.iter().all(|language| language.code != "en"));
    }

    #[tokio::test]
    async fn queued_translation_job_generates_cache_and_marks_completed() {
        let ctx = TestContext::new().await;
        sample_article("story-1", "research-bulletin")
            .insert(&ctx.state.db)
            .await
            .unwrap();
        persist_translation_job_request(
            &ctx.state,
            "story-1",
            preferred_language("pt"),
            TranslationJobRequestSource::Explicit,
            RequesterTier::Authenticated,
            "user:author@example.com",
            false,
        )
        .await
        .unwrap();

        process_translation_job_with_translator(&ctx.state, &FakeTranslator, "story-1:pt")
            .await
            .unwrap();

        let translation = load_cached_article_translation(
            &ctx.state.db,
            article_source("story-1"),
            preferred_language("pt"),
        )
        .await
        .unwrap()
        .expect("translation should be cached");
        let job = load_translation_job(&ctx.state.db, "story-1:pt")
            .await
            .unwrap()
            .expect("translation job should exist");

        assert!(translation.title.starts_with("[pt]"));
        assert_eq!(job.status, TRANSLATION_JOB_STATUS_COMPLETED);
    }

    #[tokio::test]
    async fn processing_translation_job_is_resumed_after_restart() {
        let ctx = TestContext::new().await;
        sample_article("story-2", "restart-bulletin")
            .insert(&ctx.state.db)
            .await
            .unwrap();
        persist_translation_job_request(
            &ctx.state,
            "story-2",
            preferred_language("fr"),
            TranslationJobRequestSource::Explicit,
            RequesterTier::Authenticated,
            "user:author@example.com",
            false,
        )
        .await
        .unwrap();
        let mut active: translation_job::ActiveModel =
            load_translation_job(&ctx.state.db, "story-2:fr")
                .await
                .unwrap()
                .expect("translation job should exist")
                .into();
        active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_PROCESSING.to_string());
        active.started_at = ActiveValue::set(Some(chrono::Utc::now().naive_utc()));
        active.update(&ctx.state.db).await.unwrap();

        let restarted_state = test_state_for(&ctx.db.url).await;
        let due = due_translation_jobs(&restarted_state).await.unwrap();
        assert!(due.iter().any(|job| job.id == "story-2:fr"));

        process_translation_job_with_translator(&restarted_state, &FakeTranslator, "story-2:fr")
            .await
            .unwrap();

        let translation = load_cached_article_translation(
            &restarted_state.db,
            article_source("story-2"),
            preferred_language("fr"),
        )
        .await
        .unwrap()
        .expect("translation should be cached after restart");
        let resumed_job = TranslationJob::find_by_id("story-2:fr".to_string())
            .one(&restarted_state.db)
            .await
            .unwrap()
            .expect("translation job should remain persisted");

        assert!(translation.markdown.starts_with("[fr]"));
        assert_eq!(resumed_job.status, TRANSLATION_JOB_STATUS_COMPLETED);
    }
}
