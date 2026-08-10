use std::future::Future;
use std::sync::atomic::Ordering;

use sea_orm::{ActiveModelTrait, ActiveValue};
use tokio::sync::OwnedSemaphorePermit;
use tracing::{event, Level};

use crate::audit::log_system_audit;
use crate::create::clarify::{append_clarification_fallback, parse_clarification_request};
use crate::entities::{article_job, content};
use crate::error::Error;
use crate::image_jobs::enqueue_pending_images;

use super::definitions::{
    is_in_progress_job_status, is_terminal_job_status, ArticleJobService, ArticleJobTrace,
    ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, ARTICLE_JOB_PHASE_COMPLETED, ARTICLE_JOB_PHASE_FAILED,
    ARTICLE_JOB_PHASE_QUEUED, ARTICLE_JOB_PHASE_RENDERING_IMAGES, ARTICLE_JOB_PHASE_WRITING,
    ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED, ARTICLE_JOB_STATUS_FAILED,
    ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
};
use super::support::{
    build_generation_future, build_job_preview_payload, load_article_image_progress,
    load_article_job, load_article_job_for_article, load_job_article_content, log_job_transition,
    merge_job_usage_counters, now,
};

impl ArticleJobService {
    pub async fn ensure_job_progress(&self, id: &str) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = self.load_job(id).await? else {
            return Ok(None);
        };
        let job = if job.phase == ARTICLE_JOB_PHASE_AWAITING_USER_INPUT {
            self.maybe_resume_clarification_timeout(job).await?
        } else {
            self.reconcile_job_model(job, true).await?
        };

        if is_in_progress_job_status(&job.status)
            && job.phase != ARTICLE_JOB_PHASE_RENDERING_IMAGES
            && job.phase != ARTICLE_JOB_PHASE_AWAITING_USER_INPUT
        {
            let _ = self.start_generation_job_from_model(job.clone()).await?;
        }

        self.load_job(id).await
    }

    pub async fn reconcile_job_state(&self, id: &str) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = self.load_job(id).await? else {
            return Ok(None);
        };
        self.reconcile_job_model(job, true).await.map(Some)
    }

    pub async fn reconcile_job_state_for_article(
        &self,
        article_id: &str,
    ) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = load_article_job_for_article(&self.state, article_id).await? else {
            return Ok(None);
        };
        self.reconcile_job_model(job, true).await.map(Some)
    }

    pub async fn finalize_job_state_for_article(
        &self,
        article_id: &str,
    ) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = load_article_job_for_article(&self.state, article_id).await? else {
            return Ok(None);
        };
        self.reconcile_job_model(job, false).await.map(Some)
    }

    pub async fn spawn_generation_job<F>(
        &self,
        id: String,
        permit: OwnedSemaphorePermit,
        trace: ArticleJobTrace,
        future: F,
    ) where
        F: Future<Output = Result<(), Error>> + Send + 'static,
    {
        if !self.state.try_mark_generation_started(&id).await {
            return;
        }

        let service = self.clone();
        let state = self.state.clone();

        tokio::spawn(async move {
            let _permit = permit;
            let in_flight = state
                .active_article_generations
                .fetch_add(1, Ordering::SeqCst)
                + 1;
            log_job_transition("started", &id, &trace, in_flight);

            if let Err(err) = service
                .mark_job_processing_by_id(&id, ARTICLE_JOB_PHASE_WRITING)
                .await
            {
                if service.is_job_cancelled(&id).await.unwrap_or(false) {
                    let in_flight_after = state
                        .active_article_generations
                        .fetch_sub(1, Ordering::SeqCst)
                        .saturating_sub(1);
                    log_job_transition("worker_cancelled", &id, &trace, in_flight_after);
                    return;
                }
                event!(
                    Level::ERROR,
                    job_id = %id,
                    error = %err,
                    "Failed to mark article job as processing"
                );
                let _ = service.mark_job_failed_by_id(&id, &err).await;
                let in_flight_after = state
                    .active_article_generations
                    .fetch_sub(1, Ordering::SeqCst)
                    .saturating_sub(1);
                log_job_transition("worker_finished", &id, &trace, in_flight_after);
                return;
            }

            let result = future.await;
            let in_flight_after = state
                .active_article_generations
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);

            match result {
                Ok(()) => {
                    if let Err(err) = service.reconcile_job_state(&id).await {
                        event!(
                            Level::ERROR,
                            job_id = %id,
                            error = %err,
                            "Failed to reconcile article job after generation"
                        );
                        let _ = service.mark_job_failed_by_id(&id, &err).await;
                    }
                }
                Err(err) => {
                    if service.is_job_cancelled(&id).await.unwrap_or(false) {
                        event!(
                            Level::INFO,
                            job_id = %id,
                            "Article generation job was cancelled before completion"
                        );
                        log_job_transition("worker_cancelled", &id, &trace, in_flight_after);
                        return;
                    }
                    event!(
                        Level::ERROR,
                        job_id = %id,
                        error = %err,
                        "Article generation job failed"
                    );
                    let _ = service.mark_job_failed_by_id(&id, &err).await;
                }
            }

            log_job_transition("worker_finished", &id, &trace, in_flight_after);
        });
    }

    async fn reconcile_job_model(
        &self,
        job: article_job::Model,
        enqueue_missing_images: bool,
    ) -> Result<article_job::Model, Error> {
        if is_terminal_job_status(&job.status) {
            self.state.mark_generation_finished(&job.id).await;
            return Ok(job);
        }

        let Some(article) = load_job_article_content(&self.state, &job).await? else {
            if job.phase == ARTICLE_JOB_PHASE_RENDERING_IMAGES {
                let err = Error::NotFound(Some(format!(
                    "Article content is missing for article job {}",
                    job.id
                )));
                return self.mark_job_failed(job, &err).await;
            }
            return Ok(job);
        };

        if article.markdown.is_none() {
            return Ok(job);
        }

        let progress = load_article_image_progress(&self.state, &article.id).await?;
        if progress.has_pending() {
            if enqueue_missing_images {
                enqueue_pending_images(self.state.clone(), progress.pending_ids.clone()).await;
            }
            return self
                .mark_job_rendering_images(job, &article, &progress)
                .await;
        }

        self.mark_job_completed(job, &article, &progress).await
    }

    async fn start_generation_job_from_model(
        &self,
        job: article_job::Model,
    ) -> Result<bool, Error> {
        if !is_in_progress_job_status(&job.status)
            || job.phase == ARTICLE_JOB_PHASE_RENDERING_IMAGES
        {
            return Ok(false);
        }

        let permit = match self
            .state
            .article_generation_semaphore
            .clone()
            .try_acquire_owned()
        {
            Ok(permit) => permit,
            Err(_) => return Ok(false),
        };

        let trace = ArticleJobTrace::from_job(&job);
        let future = build_generation_future(self.state.clone(), &job)?;
        self.spawn_generation_job(job.id.clone(), permit, trace, future)
            .await;
        Ok(true)
    }

    async fn mark_job_processing_by_id(&self, id: &str, phase: &str) -> Result<(), Error> {
        let Some(job) = load_article_job(&self.state, id).await? else {
            return Err(Error::NotFound(Some(format!(
                "Article job {} not found",
                id
            ))));
        };
        if job.status == ARTICLE_JOB_STATUS_CANCELLED {
            return Err(Error::BadRequest(format!(
                "Article job {} was cancelled",
                id
            )));
        }
        self.mark_job_processing(job, phase).await?;
        Ok(())
    }

    async fn mark_job_failed_by_id(&self, id: &str, err: &Error) -> Result<(), Error> {
        let Some(job) = load_article_job(&self.state, id).await? else {
            self.state.mark_generation_finished(id).await;
            return Ok(());
        };
        self.mark_job_failed(job, err).await?;
        Ok(())
    }

    async fn maybe_resume_clarification_timeout(
        &self,
        job: article_job::Model,
    ) -> Result<article_job::Model, Error> {
        let Some(clarification) = parse_clarification_request(job.preview_payload.as_deref())
        else {
            return Ok(job);
        };
        let Some(auto_resume_at) = clarification.auto_resume_at_datetime() else {
            return Ok(job);
        };
        if auto_resume_at > now() {
            return Ok(job);
        }

        let prompt =
            append_clarification_fallback(&job.prompt, &clarification.fallback_instruction);
        let updated = self.resume_job_with_prompt(job.clone(), prompt).await?;
        let details = serde_json::json!({
            "question": clarification.question,
            "auto_resume_at": clarification.auto_resume_at,
        })
        .to_string();
        log_system_audit(
            &self.state.db,
            "article_job_clarification_timed_out",
            "article_job",
            &job.id,
            Some(details),
        )
        .await?;
        Ok(updated)
    }

    async fn mark_job_processing(
        &self,
        job: article_job::Model,
        phase: &str,
    ) -> Result<article_job::Model, Error> {
        let mut active: article_job::ActiveModel = job.into();
        let reference_time = now();
        active.phase = ActiveValue::set(phase.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_PROCESSING.to_string());
        active.error_summary = ActiveValue::set(None);
        active.updated_at = ActiveValue::set(reference_time);
        if active.started_at.is_not_set() {
            active.started_at = ActiveValue::set(Some(reference_time));
        }
        active
            .update(&self.state.db)
            .await
            .map_err(|e| Error::Database(format!("Error marking article job as processing: {}", e)))
    }

    pub(super) async fn resume_job_with_prompt(
        &self,
        job: article_job::Model,
        prompt: String,
    ) -> Result<article_job::Model, Error> {
        let mut active: article_job::ActiveModel = job.into();
        let reference_time = now();
        active.prompt = ActiveValue::set(prompt);
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_QUEUED.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_QUEUED.to_string());
        active.preview_payload = ActiveValue::set(None);
        active.error_summary = ActiveValue::set(None);
        active.finished_at = ActiveValue::set(None);
        active.updated_at = ActiveValue::set(reference_time);
        active.update(&self.state.db).await.map_err(|e| {
            Error::Database(format!(
                "Error resuming article job from clarification: {}",
                e
            ))
        })
    }

    async fn mark_job_rendering_images(
        &self,
        job: article_job::Model,
        article: &content::Model,
        progress: &super::definitions::ImageProgress,
    ) -> Result<article_job::Model, Error> {
        let mut active: article_job::ActiveModel = job.clone().into();
        let reference_time = now();
        active.article_id = ActiveValue::set(Some(article.id.clone()));
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_RENDERING_IMAGES.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_PROCESSING.to_string());
        active.preview_payload = ActiveValue::set(Some(build_job_preview_payload(
            job.preview_payload.as_deref(),
            article,
            progress,
        )));
        active.usage_counters = ActiveValue::set(Some(merge_job_usage_counters(
            job.usage_counters.as_deref(),
            &job.prompt,
            progress,
        )));
        active.error_summary = ActiveValue::set(None);
        active.updated_at = ActiveValue::set(reference_time);
        if active.started_at.is_not_set() {
            active.started_at = ActiveValue::set(Some(reference_time));
        }
        let updated = active.update(&self.state.db).await.map_err(|e| {
            Error::Database(format!(
                "Error marking article job as rendering images: {}",
                e
            ))
        })?;
        self.state.mark_generation_started(&job.id).await;
        Ok(updated)
    }

    async fn mark_job_completed(
        &self,
        job: article_job::Model,
        article: &content::Model,
        progress: &super::definitions::ImageProgress,
    ) -> Result<article_job::Model, Error> {
        let mut active: article_job::ActiveModel = job.clone().into();
        let reference_time = now();
        active.article_id = ActiveValue::set(Some(article.id.clone()));
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_COMPLETED.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_COMPLETED.to_string());
        active.preview_payload = ActiveValue::set(Some(build_job_preview_payload(
            job.preview_payload.as_deref(),
            article,
            progress,
        )));
        active.usage_counters = ActiveValue::set(Some(merge_job_usage_counters(
            job.usage_counters.as_deref(),
            &job.prompt,
            progress,
        )));
        active.error_summary = ActiveValue::set(None);
        active.finished_at = ActiveValue::set(Some(reference_time));
        active.updated_at = ActiveValue::set(reference_time);
        let updated = active.update(&self.state.db).await.map_err(|e| {
            Error::Database(format!("Error marking article job as completed: {}", e))
        })?;

        self.state.mark_generation_finished(&job.id).await;
        let details = serde_json::json!({
            "article_id": article.id,
            "slug": article.slug,
            "feature_type": job.feature_type,
            "image_total": progress.total,
            "image_failed": progress.failed,
        })
        .to_string();
        log_system_audit(
            &self.state.db,
            "article_job_completed",
            "article_job",
            &job.id,
            Some(details),
        )
        .await?;
        Ok(updated)
    }

    async fn mark_job_failed(
        &self,
        job: article_job::Model,
        err: &Error,
    ) -> Result<article_job::Model, Error> {
        let mut active: article_job::ActiveModel = job.clone().into();
        let reference_time = now();
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_FAILED.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_FAILED.to_string());
        active.error_summary = ActiveValue::set(Some(err.to_string()));
        active.fail_count = ActiveValue::set(job.fail_count + 1);
        active.finished_at = ActiveValue::set(Some(reference_time));
        active.updated_at = ActiveValue::set(reference_time);
        let updated = active
            .update(&self.state.db)
            .await
            .map_err(|e| Error::Database(format!("Error marking article job as failed: {}", e)))?;

        self.state.mark_generation_finished(&job.id).await;
        let details = serde_json::json!({
            "article_id": job.article_id,
            "feature_type": job.feature_type,
            "error": err.to_string(),
            "fail_count": job.fail_count + 1,
        })
        .to_string();
        log_system_audit(
            &self.state.db,
            "article_job_failed",
            "article_job",
            &job.id,
            Some(details),
        )
        .await?;
        Ok(updated)
    }
}
