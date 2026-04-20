use std::time::Duration;

use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::article_id::normalize_content_model;
use crate::audit::log_system_audit;
use crate::entities::{prelude::*, translation_job};
use crate::error::Error;
use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};
use crate::llm::translate::Translate;
use crate::rate_limit::RequesterTier;
use crate::services::article_translations::{
    ensure_cached_article_translation, load_cached_article_translation, owned_article_source_text,
};

use super::definitions::{
    TranslationJobRequestSource, TRANSLATION_JOB_STATUS_CANCELLED,
    TRANSLATION_JOB_STATUS_COMPLETED, TRANSLATION_JOB_STATUS_FAILED,
    TRANSLATION_JOB_STATUS_PROCESSING, TRANSLATION_JOB_STATUS_QUEUED,
};
use super::queue::{
    delete_translation_job, due_translation_jobs, load_translation_job,
    persist_translation_job_request,
};
use super::support::{
    now, translation_resume_interval_seconds, translation_retry_delay,
    translation_retry_max_chrono_seconds,
};

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
            .unwrap_or_else(|_| chrono::Duration::seconds(translation_retry_max_chrono_seconds()));
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

pub(super) async fn process_translation_job_with_translator<T: Translate>(
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

pub(super) async fn spawn_due_translation_jobs(state: AppState) {
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
