use std::sync::atomic::Ordering;

use tracing::{event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::Error;
use crate::rate_limit::ArticleRateLimit;

use super::{create_article, normalize_create_prompt};

pub async fn start_create_article(
    state: AppState,
    prompt: String,
    author_email: Option<String>,
) -> Result<String, Error> {
    let prompt = normalize_create_prompt(&prompt)?;
    let permit = state
        .article_generation_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            event!(
                Level::WARN,
                "Rejected article creation due to concurrency limit (MAX_CONCURRENT_ARTICLE_GENERATIONS reached)",
            );
            Error::RateLimited
        })?;

    state
        .rate_limit_state
        .check_article_generation_limit()
        .map_err(|limit| {
            let limit_name = match limit {
                ArticleRateLimit::Hourly => "hourly",
                ArticleRateLimit::Daily => "daily",
            };
            event!(
                Level::WARN,
                limit = limit_name,
                "Rejected article creation due to article generation rate limit",
            );
            Error::RateLimited
        })?;

    let id = Uuid::new_v4().to_string();
    event!(Level::DEBUG, "Created id {}", &id);
    let return_id = id.clone();
    let active_counter = state.active_article_generations.clone();
    state.mark_generation_started(&id).await;
    state
        .task_list
        .clone()
        .spawn_task(id.clone(), async move {
            let _permit = permit;
            let in_flight = active_counter.fetch_add(1, Ordering::SeqCst) + 1;
            event!(
                Level::INFO,
                article_id = %id,
                in_flight,
                "Started article generation task"
            );
            let result = create_article(&state, id.clone(), prompt.clone(), author_email).await;
            let in_flight_after = active_counter
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);
            event!(
                Level::INFO,
                article_id = %id,
                in_flight = in_flight_after,
                "Finished article generation task"
            );
            state.mark_generation_finished(&id).await;
            result
        })
        .await;
    Ok(return_id)
}
