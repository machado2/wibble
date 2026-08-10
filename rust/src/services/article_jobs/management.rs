use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};
use serde_json::Value;
use tokio::sync::OwnedSemaphorePermit;
use tracing::{event, Level};
use uuid::Uuid;

use crate::audit::log_system_audit;
use crate::create::clarify::{append_clarification_answer, parse_clarification_request};
use crate::entities::article_job;
use crate::entities::prelude::ArticleJob;
use crate::error::Error;
use crate::rate_limit::{ArticleRateLimit, RequesterTier};

use super::definitions::{
    is_terminal_job_status, ArticleJobRequest, ArticleJobService,
    ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, ARTICLE_JOB_PHASE_CANCELLED, ARTICLE_JOB_PHASE_QUEUED,
    ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
};
use super::support::{default_usage_counters, load_article_job, now};

impl ArticleJobService {
    pub fn new(state: crate::app_state::AppState) -> Self {
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
}
