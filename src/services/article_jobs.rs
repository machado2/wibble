use std::future::Future;
use std::sync::atomic::Ordering;

use tokio::sync::OwnedSemaphorePermit;
use tracing::{event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::Error;
use crate::rate_limit::{ArticleRateLimit, RequesterTier};
use crate::tasklist::TaskResult;

#[derive(Clone)]
pub struct ArticleJobService {
    state: AppState,
}

#[derive(Clone, Debug)]
pub struct ArticleJobTrace {
    pub job_kind: &'static str,
    pub recovery_slug: Option<String>,
}

impl ArticleJobTrace {
    pub fn create() -> Self {
        Self {
            job_kind: "create",
            recovery_slug: None,
        }
    }

    pub fn dead_link_recovery(slug: String) -> Self {
        Self {
            job_kind: "dead_link_recovery",
            recovery_slug: Some(slug),
        }
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

    pub async fn task_result(&self, id: &str) -> Result<TaskResult, Error> {
        self.state.task_list.get(id).await
    }

    pub async fn is_job_processing(&self, article_id: &str) -> bool {
        if self.state.is_generation_active(article_id).await {
            return true;
        }

        matches!(
            self.task_result(article_id).await,
            Ok(TaskResult::Processing)
        )
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
        let state = self.state.clone();
        let active_counter = state.active_article_generations.clone();
        state.mark_generation_started(&id).await;
        state
            .task_list
            .clone()
            .spawn_task(id.clone(), async move {
                let _permit = permit;
                let in_flight = active_counter.fetch_add(1, Ordering::SeqCst) + 1;
                log_job_transition("started", &id, &trace, in_flight);

                let result = future.await;
                let in_flight_after = active_counter
                    .fetch_sub(1, Ordering::SeqCst)
                    .saturating_sub(1);

                log_job_transition("finished", &id, &trace, in_flight_after);
                state.mark_generation_finished(&id).await;
                result
            })
            .await;
    }
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
