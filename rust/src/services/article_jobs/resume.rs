use std::time::Duration;

use tracing::{event, Level};

use crate::app_state::AppState;

use super::definitions::ArticleJobService;
use super::support::{article_job_resume_interval_seconds, due_article_job_ids};

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
