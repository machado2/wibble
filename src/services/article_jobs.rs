use std::env;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::time::Duration;

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde_json::Value;
use tokio::sync::OwnedSemaphorePermit;
use tracing::{event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::audit::log_system_audit;
use crate::create::clarify::{
    append_clarification_answer, append_clarification_fallback, parse_clarification_request,
};
use crate::create::create_article;
use crate::entities::{article_job, content, content_image, prelude::*};
use crate::error::Error;
use crate::image_jobs::enqueue_pending_images;
use crate::image_status::{is_pending_status, IMAGE_STATUS_COMPLETED, IMAGE_STATUS_FAILED};
use crate::llm::article_generator::ResearchModeSource;
use crate::rate_limit::{ArticleRateLimit, RequesterTier};

pub const ARTICLE_JOB_STATUS_QUEUED: &str = "queued";
pub const ARTICLE_JOB_STATUS_PROCESSING: &str = "processing";
pub const ARTICLE_JOB_STATUS_COMPLETED: &str = "completed";
pub const ARTICLE_JOB_STATUS_FAILED: &str = "failed";
pub const ARTICLE_JOB_STATUS_CANCELLED: &str = "cancelled";

pub const ARTICLE_JOB_PHASE_QUEUED: &str = "queued";
pub const ARTICLE_JOB_PHASE_PLANNING: &str = "planning";
pub const ARTICLE_JOB_PHASE_RESEARCHING: &str = "researching";
pub const ARTICLE_JOB_PHASE_AWAITING_USER_INPUT: &str = "awaiting_user_input";
pub const ARTICLE_JOB_PHASE_WRITING: &str = "writing";
pub const ARTICLE_JOB_PHASE_EDITING: &str = "editing";
pub const ARTICLE_JOB_PHASE_TRANSLATING: &str = "translating";
pub const ARTICLE_JOB_PHASE_RENDERING_IMAGES: &str = "rendering_images";
pub const ARTICLE_JOB_PHASE_READY_FOR_REVIEW: &str = "ready_for_review";
pub const ARTICLE_JOB_PHASE_COMPLETED: &str = "completed";
pub const ARTICLE_JOB_PHASE_FAILED: &str = "failed";
pub const ARTICLE_JOB_PHASE_CANCELLED: &str = "cancelled";

type BoxedArticleFuture = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

#[derive(Clone)]
pub struct ArticleJobService {
    state: AppState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArticleJobFeatureType {
    Create,
    CreateResearchAuto,
    CreateResearchManual,
    DeadLinkRecovery,
}

impl ArticleJobFeatureType {
    fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::CreateResearchAuto => "create_research_auto",
            Self::CreateResearchManual => "create_research_manual",
            Self::DeadLinkRecovery => "dead_link_recovery",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "create_research_auto" => Some(Self::CreateResearchAuto),
            "create_research_manual" => Some(Self::CreateResearchManual),
            "dead_link_recovery" => Some(Self::DeadLinkRecovery),
            _ => None,
        }
    }

    fn from_research_mode(mode: Option<ResearchModeSource>) -> Self {
        match mode {
            Some(ResearchModeSource::Auto) => Self::CreateResearchAuto,
            Some(ResearchModeSource::Manual) => Self::CreateResearchManual,
            None => Self::Create,
        }
    }

    fn research_mode(self) -> Option<ResearchModeSource> {
        match self {
            Self::Create => None,
            Self::CreateResearchAuto => Some(ResearchModeSource::Auto),
            Self::CreateResearchManual => Some(ResearchModeSource::Manual),
            Self::DeadLinkRecovery => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArticleJobRequest {
    article_id: Option<String>,
    requester_key: String,
    requester_tier: String,
    author_email: Option<String>,
    prompt: String,
    feature_type: ArticleJobFeatureType,
}

impl ArticleJobRequest {
    pub fn create(
        prompt: String,
        author_email: Option<String>,
        requester_tier: RequesterTier,
        rate_limit_key: String,
        research_mode: Option<ResearchModeSource>,
    ) -> Self {
        Self {
            article_id: None,
            requester_key: rate_limit_key,
            requester_tier: requester_tier_label(requester_tier).to_string(),
            author_email,
            prompt,
            feature_type: ArticleJobFeatureType::from_research_mode(research_mode),
        }
    }

    pub fn dead_link_recovery(prompt: String, article_id: String) -> Self {
        Self {
            article_id: Some(article_id),
            requester_key: "system:dead_link_recovery".to_string(),
            requester_tier: "SYSTEM".to_string(),
            author_email: None,
            prompt,
            feature_type: ArticleJobFeatureType::DeadLinkRecovery,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArticleJobTrace {
    pub job_kind: &'static str,
    pub recovery_slug: Option<String>,
}

impl ArticleJobTrace {
    pub fn create(research_mode: Option<ResearchModeSource>) -> Self {
        let feature_type = ArticleJobFeatureType::from_research_mode(research_mode);
        Self {
            job_kind: feature_type.as_str(),
            recovery_slug: None,
        }
    }

    pub fn dead_link_recovery(slug: String) -> Self {
        Self {
            job_kind: ArticleJobFeatureType::DeadLinkRecovery.as_str(),
            recovery_slug: Some(slug),
        }
    }

    fn from_job(job: &article_job::Model) -> Self {
        match ArticleJobFeatureType::from_str(&job.feature_type) {
            Some(ArticleJobFeatureType::DeadLinkRecovery) => Self {
                job_kind: ArticleJobFeatureType::DeadLinkRecovery.as_str(),
                recovery_slug: None,
            },
            Some(feature_type) => Self {
                job_kind: feature_type.as_str(),
                recovery_slug: None,
            },
            None => Self::create(None),
        }
    }
}

#[derive(Clone, Debug, Default)]
struct ImageProgress {
    total: usize,
    completed: usize,
    processing: usize,
    failed: usize,
    pending_ids: Vec<String>,
}

impl ImageProgress {
    fn has_pending(&self) -> bool {
        !self.pending_ids.is_empty()
    }
}

impl ArticleJobService {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub fn new_job_id(&self) -> String {
        Uuid::new_v4().to_string()
    }

    pub fn try_acquire_generation_slot(
        &self,
        operation: &'static str,
    ) -> Result<OwnedSemaphorePermit, Error> {
        self.state
            .article_generation_semaphore
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                event!(
                    Level::WARN,
                    operation,
                    "Rejected article job due to concurrency limit (MAX_CONCURRENT_ARTICLE_GENERATIONS reached)",
                );
                Error::RateLimited
            })
    }

    pub fn check_create_rate_limit(
        &self,
        requester_tier: RequesterTier,
        rate_limit_key: &str,
    ) -> Result<(), Error> {
        self.state
            .rate_limit_state
            .check_article_generation_limit(requester_tier, rate_limit_key)
            .map_err(|limit| {
                let limit_name = match limit {
                    ArticleRateLimit::Hourly => "hourly",
                    ArticleRateLimit::Daily => "daily",
                };
                event!(
                    Level::WARN,
                    limit = limit_name,
                    tier = ?requester_tier,
                    "Rejected article creation due to article generation rate limit",
                );
                Error::RateLimited
            })
    }

    pub fn check_research_rate_limit(
        &self,
        requester_tier: RequesterTier,
        rate_limit_key: &str,
    ) -> Result<(), Error> {
        self.state
            .rate_limit_state
            .check_research_generation_limit(requester_tier, rate_limit_key)
            .map_err(|limit| {
                let limit_name = match limit {
                    ArticleRateLimit::Hourly => "hourly",
                    ArticleRateLimit::Daily => "daily",
                };
                event!(
                    Level::WARN,
                    limit = limit_name,
                    tier = ?requester_tier,
                    "Rejected article creation due to research generation rate limit",
                );
                Error::RateLimited
            })
    }

    pub async fn create_job(&self, id: String, request: ArticleJobRequest) -> Result<(), Error> {
        let prompt_chars = request.prompt.chars().count();
        let prompt = request.prompt;
        let reference_time = now();

        ArticleJob::insert(article_job::ActiveModel {
            id: ActiveValue::set(id),
            article_id: ActiveValue::set(request.article_id),
            requester_key: ActiveValue::set(request.requester_key),
            requester_tier: ActiveValue::set(request.requester_tier),
            author_email: ActiveValue::set(request.author_email),
            prompt: ActiveValue::set(prompt),
            feature_type: ActiveValue::set(request.feature_type.as_str().to_string()),
            phase: ActiveValue::set(ARTICLE_JOB_PHASE_QUEUED.to_string()),
            status: ActiveValue::set(ARTICLE_JOB_STATUS_QUEUED.to_string()),
            usage_counters: ActiveValue::set(Some(
                default_usage_counters(prompt_chars).to_string(),
            )),
            preview_payload: ActiveValue::set(None),
            error_summary: ActiveValue::set(None),
            fail_count: ActiveValue::set(0),
            created_at: ActiveValue::set(reference_time),
            updated_at: ActiveValue::set(reference_time),
            started_at: ActiveValue::set(None),
            finished_at: ActiveValue::set(None),
        })
        .exec(&self.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting article job: {}", e)))?;

        Ok(())
    }

    pub async fn record_usage_snapshot(
        &self,
        id: &str,
        phase: &str,
        usage: Value,
    ) -> Result<(), Error> {
        let Some(job) = self.load_job(id).await? else {
            return Err(Error::NotFound(Some(format!(
                "Article job {} not found",
                id
            ))));
        };

        let was_queued = job.status == ARTICLE_JOB_STATUS_QUEUED;
        let mut active: article_job::ActiveModel = job.into();
        let reference_time = now();
        active.usage_counters = ActiveValue::set(Some(usage.to_string()));
        active.updated_at = ActiveValue::set(reference_time);
        active.phase = ActiveValue::set(phase.to_string());
        if was_queued {
            active.status = ActiveValue::set(ARTICLE_JOB_STATUS_PROCESSING.to_string());
        }
        if active.started_at.is_not_set() {
            active.started_at = ActiveValue::set(Some(reference_time));
        }
        active.update(&self.state.db).await.map_err(|e| {
            Error::Database(format!("Error recording article job usage snapshot: {}", e))
        })?;
        Ok(())
    }

    pub async fn load_job(&self, id: &str) -> Result<Option<article_job::Model>, Error> {
        load_article_job(&self.state, id).await
    }

    pub async fn is_job_cancelled(&self, id: &str) -> Result<bool, Error> {
        Ok(self
            .load_job(id)
            .await?
            .is_some_and(|job| job.status == ARTICLE_JOB_STATUS_CANCELLED))
    }

    pub async fn cancel_job(
        &self,
        id: &str,
        reason: &str,
    ) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = self.load_job(id).await? else {
            return Ok(None);
        };
        if job.status == ARTICLE_JOB_STATUS_CANCELLED {
            return Ok(Some(job));
        }
        if is_terminal_job_status(&job.status) {
            return Err(Error::BadRequest(format!(
                "Article job {} is already {}",
                id, job.status
            )));
        }

        let mut active: article_job::ActiveModel = job.into();
        let reference_time = now();
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_CANCELLED.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_CANCELLED.to_string());
        active.error_summary = ActiveValue::set(Some(reason.to_string()));
        active.finished_at = ActiveValue::set(Some(reference_time));
        active.updated_at = ActiveValue::set(reference_time);
        let updated = active
            .update(&self.state.db)
            .await
            .map_err(|e| Error::Database(format!("Error cancelling article job: {}", e)))?;
        self.state.mark_generation_finished(id).await;
        Ok(Some(updated))
    }

    pub async fn request_clarification(
        &self,
        id: &str,
        payload: String,
    ) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = self.load_job(id).await? else {
            return Ok(None);
        };

        let mut active: article_job::ActiveModel = job.into();
        let reference_time = now();
        active.phase = ActiveValue::set(ARTICLE_JOB_PHASE_AWAITING_USER_INPUT.to_string());
        active.status = ActiveValue::set(ARTICLE_JOB_STATUS_PROCESSING.to_string());
        active.preview_payload = ActiveValue::set(Some(payload));
        active.updated_at = ActiveValue::set(reference_time);
        if active.started_at.is_not_set() {
            active.started_at = ActiveValue::set(Some(reference_time));
        }
        let updated = active
            .update(&self.state.db)
            .await
            .map_err(|e| Error::Database(format!("Error requesting clarification: {}", e)))?;
        let details = serde_json::json!({
            "phase": ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
        })
        .to_string();
        log_system_audit(
            &self.state.db,
            "article_job_clarification_requested",
            "article_job",
            id,
            Some(details),
        )
        .await?;
        Ok(Some(updated))
    }

    pub async fn merge_preview_payload(&self, id: &str, payload: Value) -> Result<(), Error> {
        let Some(job) = self.load_job(id).await? else {
            return Err(Error::NotFound(Some(format!(
                "Article job {} not found",
                id
            ))));
        };
        let mut object = job
            .preview_payload
            .as_deref()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .and_then(|value| value.as_object().cloned())
            .unwrap_or_default();
        if let Some(payload_object) = payload.as_object() {
            for (key, value) in payload_object {
                object.insert(key.clone(), value.clone());
            }
        }

        let mut active: article_job::ActiveModel = job.into();
        active.preview_payload = ActiveValue::set(Some(Value::Object(object).to_string()));
        active.updated_at = ActiveValue::set(now());
        active.update(&self.state.db).await.map_err(|e| {
            Error::Database(format!("Error updating article job preview payload: {}", e))
        })?;
        Ok(())
    }

    pub async fn submit_clarification_answer(
        &self,
        id: &str,
        answer: &str,
    ) -> Result<Option<article_job::Model>, Error> {
        let Some(job) = self.load_job(id).await? else {
            return Ok(None);
        };
        if job.phase != ARTICLE_JOB_PHASE_AWAITING_USER_INPUT {
            return Err(Error::BadRequest(format!(
                "Article job {} is not waiting for clarification",
                id
            )));
        }
        let clarification = parse_clarification_request(job.preview_payload.as_deref())
            .ok_or_else(|| Error::BadRequest("Clarification prompt is missing".to_string()))?;
        let prompt = append_clarification_answer(&job.prompt, &clarification.question, answer);
        let updated = self.resume_job_with_prompt(job, prompt).await?;
        let details = serde_json::json!({
            "answer_chars": answer.chars().count(),
        })
        .to_string();
        log_system_audit(
            &self.state.db,
            "article_job_clarification_answered",
            "article_job",
            id,
            Some(details),
        )
        .await?;
        Ok(Some(updated))
    }

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
        let Some(job) = self.load_job(id).await? else {
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
        let Some(job) = self.load_job(id).await? else {
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

    async fn resume_job_with_prompt(
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
        progress: &ImageProgress,
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
        progress: &ImageProgress,
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

pub fn is_in_progress_job_status(status: &str) -> bool {
    matches!(
        status,
        ARTICLE_JOB_STATUS_QUEUED | ARTICLE_JOB_STATUS_PROCESSING
    )
}

pub fn is_terminal_job_status(status: &str) -> bool {
    matches!(
        status,
        ARTICLE_JOB_STATUS_COMPLETED | ARTICLE_JOB_STATUS_FAILED | ARTICLE_JOB_STATUS_CANCELLED
    )
}

pub async fn spawn_due_article_jobs(state: AppState) {
    let jobs = match due_article_job_ids(&state).await {
        Ok(jobs) => jobs,
        Err(err) => {
            event!(Level::ERROR, error = %err, "Failed to load due article jobs");
            return;
        }
    };

    let service = ArticleJobService::new(state);
    for job_id in jobs {
        if let Err(err) = service.ensure_job_progress(&job_id).await {
            event!(
                Level::ERROR,
                job_id = %job_id,
                error = %err,
                "Failed to resume article job"
            );
        }
    }
}

pub fn spawn_resume_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(article_job_resume_interval_seconds()));
        loop {
            interval.tick().await;
            spawn_due_article_jobs(state.clone()).await;
        }
    });
}

async fn due_article_job_ids(state: &AppState) -> Result<Vec<String>, Error> {
    ArticleJob::find()
        .filter(
            article_job::Column::Status
                .is_in([ARTICLE_JOB_STATUS_QUEUED, ARTICLE_JOB_STATUS_PROCESSING]),
        )
        .order_by_asc(article_job::Column::CreatedAt)
        .limit(article_job_resume_batch_size())
        .select_only()
        .column(article_job::Column::Id)
        .into_tuple::<String>()
        .all(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading pending article jobs: {}", e)))
}

async fn load_article_job(
    state: &AppState,
    job_id: &str,
) -> Result<Option<article_job::Model>, Error> {
    ArticleJob::find_by_id(job_id.to_string())
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article job {}: {}", job_id, e)))
}

async fn load_article_job_for_article(
    state: &AppState,
    article_id: &str,
) -> Result<Option<article_job::Model>, Error> {
    ArticleJob::find()
        .filter(
            Condition::any()
                .add(article_job::Column::Id.eq(article_id.to_string()))
                .add(article_job::Column::ArticleId.eq(article_id.to_string())),
        )
        .order_by_desc(article_job::Column::CreatedAt)
        .one(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!(
                "Error loading article job for article {}: {}",
                article_id, e
            ))
        })
}

async fn load_job_article_content(
    state: &AppState,
    job: &article_job::Model,
) -> Result<Option<content::Model>, Error> {
    let mut candidate_ids = Vec::new();
    if let Some(article_id) = job.article_id.as_deref() {
        candidate_ids.push(article_id.to_string());
    }
    if job.id != job.article_id.as_deref().unwrap_or_default() {
        candidate_ids.push(job.id.clone());
    }

    for article_id in candidate_ids {
        if let Some(article) = Content::find_by_id(article_id.clone())
            .one(&state.db)
            .await
            .map_err(|e| {
                Error::Database(format!(
                    "Error loading article content {}: {}",
                    article_id, e
                ))
            })?
        {
            return Ok(Some(article));
        }
    }

    Ok(None)
}

async fn load_article_image_progress(
    state: &AppState,
    article_id: &str,
) -> Result<ImageProgress, Error> {
    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(article_id.to_string()))
        .all(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!(
                "Error loading article images for article {}: {}",
                article_id, e
            ))
        })?;

    let mut progress = ImageProgress {
        total: images.len(),
        ..ImageProgress::default()
    };
    for image in images {
        if image.status == IMAGE_STATUS_COMPLETED {
            progress.completed += 1;
        } else if image.status == IMAGE_STATUS_FAILED {
            progress.failed += 1;
        } else if is_pending_status(&image.status) {
            progress.processing += 1;
            progress.pending_ids.push(image.id);
        }
    }

    Ok(progress)
}

fn build_generation_future(
    state: AppState,
    job: &article_job::Model,
) -> Result<BoxedArticleFuture, Error> {
    let Some(feature_type) = ArticleJobFeatureType::from_str(&job.feature_type) else {
        return Err(Error::BadRequest(format!(
            "Unsupported article job feature type: {}",
            job.feature_type
        )));
    };
    let job_id = job.id.clone();
    let prompt = job.prompt.clone();
    let author_email = job.author_email.clone();
    let research_mode = feature_type.research_mode();
    Ok(Box::pin(async move {
        create_article(&state, job_id, prompt, author_email, research_mode).await
    }))
}

fn requester_tier_label(requester_tier: RequesterTier) -> &'static str {
    match requester_tier {
        RequesterTier::Anonymous => "ANON",
        RequesterTier::Authenticated => "AUTH",
        RequesterTier::Admin => "ADMIN",
    }
}

fn default_usage_counters(prompt_chars: usize) -> Value {
    serde_json::json!({
        "prompt_chars": prompt_chars,
        "agent_steps": 0,
        "model_calls": 0,
        "tool_calls": 0,
        "searches": 0,
        "sources": 0,
        "fetched_content_chars": 0,
        "image_total": 0,
        "image_completed": 0,
        "image_processing": 0,
        "image_failed": 0,
    })
}

fn parse_usage_counters(existing: Option<&str>, prompt_chars: usize) -> Value {
    existing
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .filter(|value| value.is_object())
        .unwrap_or_else(|| default_usage_counters(prompt_chars))
}

fn merge_job_usage_counters(
    existing: Option<&str>,
    prompt: &str,
    progress: &ImageProgress,
) -> String {
    let prompt_chars = prompt.chars().count();
    let mut value = parse_usage_counters(existing, prompt_chars);
    let object = value
        .as_object_mut()
        .expect("usage counter payload must stay an object");
    object.insert("prompt_chars".to_string(), Value::from(prompt_chars));
    object.insert(
        "image_total".to_string(),
        Value::from(progress.total as u64),
    );
    object.insert(
        "image_completed".to_string(),
        Value::from(progress.completed as u64),
    );
    object.insert(
        "image_processing".to_string(),
        Value::from(progress.processing as u64),
    );
    object.insert(
        "image_failed".to_string(),
        Value::from(progress.failed as u64),
    );
    value.to_string()
}

fn build_job_preview_payload(
    existing: Option<&str>,
    article: &content::Model,
    progress: &ImageProgress,
) -> String {
    let mut payload = existing
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let publication_state = if article.published { "public" } else { "draft" };
    payload.insert("article_id".to_string(), Value::from(article.id.clone()));
    payload.insert("slug".to_string(), Value::from(article.slug.clone()));
    payload.insert("title".to_string(), Value::from(article.title.clone()));
    payload.insert(
        "publication_state".to_string(),
        Value::from(publication_state),
    );
    payload.insert(
        "image_total".to_string(),
        Value::from(progress.total as u64),
    );
    payload.insert(
        "image_completed".to_string(),
        Value::from(progress.completed as u64),
    );
    payload.insert(
        "image_failed".to_string(),
        Value::from(progress.failed as u64),
    );
    Value::Object(payload).to_string()
}

fn article_job_resume_interval_seconds() -> u64 {
    env::var("ARTICLE_JOB_RESUME_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(10)
}

fn article_job_resume_batch_size() -> u64 {
    env::var("ARTICLE_JOB_RESUME_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50)
}

fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_local()
}

fn log_job_transition(
    stage: &'static str,
    article_id: &str,
    trace: &ArticleJobTrace,
    in_flight: usize,
) {
    event!(
        Level::INFO,
        article_id,
        job_kind = trace.job_kind,
        recovery_slug = trace.recovery_slug.as_deref().unwrap_or(""),
        stage,
        in_flight,
        "Article generation job state changed"
    );
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{
        default_usage_counters, is_in_progress_job_status, is_terminal_job_status,
        merge_job_usage_counters, ImageProgress, ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
        ARTICLE_JOB_PHASE_CANCELLED, ARTICLE_JOB_PHASE_COMPLETED, ARTICLE_JOB_PHASE_EDITING,
        ARTICLE_JOB_PHASE_FAILED, ARTICLE_JOB_PHASE_PLANNING, ARTICLE_JOB_PHASE_QUEUED,
        ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RENDERING_IMAGES,
        ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_TRANSLATING, ARTICLE_JOB_PHASE_WRITING,
        ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED, ARTICLE_JOB_STATUS_FAILED,
        ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
    };

    #[test]
    fn explicit_phase_constants_cover_persisted_agent_lifecycle() {
        let phases = [
            ARTICLE_JOB_PHASE_QUEUED,
            ARTICLE_JOB_PHASE_PLANNING,
            ARTICLE_JOB_PHASE_RESEARCHING,
            ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
            ARTICLE_JOB_PHASE_WRITING,
            ARTICLE_JOB_PHASE_EDITING,
            ARTICLE_JOB_PHASE_TRANSLATING,
            ARTICLE_JOB_PHASE_RENDERING_IMAGES,
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW,
            ARTICLE_JOB_PHASE_COMPLETED,
            ARTICLE_JOB_PHASE_FAILED,
            ARTICLE_JOB_PHASE_CANCELLED,
        ];

        assert!(phases.contains(&ARTICLE_JOB_PHASE_RENDERING_IMAGES));
        assert!(phases.contains(&ARTICLE_JOB_PHASE_READY_FOR_REVIEW));
    }

    #[test]
    fn job_status_helpers_distinguish_terminal_and_active_states() {
        assert!(is_in_progress_job_status(ARTICLE_JOB_STATUS_QUEUED));
        assert!(is_in_progress_job_status(ARTICLE_JOB_STATUS_PROCESSING));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_COMPLETED));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_FAILED));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_CANCELLED));
        assert!(!is_terminal_job_status(ARTICLE_JOB_STATUS_QUEUED));
    }

    #[test]
    fn usage_counters_include_runtime_and_image_progress() {
        let mut usage = default_usage_counters(14);
        usage
            .as_object_mut()
            .unwrap()
            .insert("model_calls".to_string(), Value::from(2));
        let json = merge_job_usage_counters(
            Some(&usage.to_string()),
            "deadpan prompt",
            &ImageProgress {
                total: 3,
                completed: 1,
                processing: 1,
                failed: 1,
                pending_ids: vec!["img-2".to_string()],
            },
        );

        assert!(json.contains("\"prompt_chars\":14"));
        assert!(json.contains("\"model_calls\":2"));
        assert!(json.contains("\"image_total\":3"));
        assert!(json.contains("\"image_failed\":1"));
    }
}
