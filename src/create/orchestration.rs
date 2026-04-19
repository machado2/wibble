use tracing::{event, Level};

use crate::app_state::AppState;
use crate::error::Error;
use crate::rate_limit::RequesterTier;
use crate::services::article_jobs::{ArticleJobService, ArticleJobTrace};

use super::{create_article, normalize_create_prompt};

pub async fn start_create_article(
    state: AppState,
    prompt: String,
    author_email: Option<String>,
    requester_tier: RequesterTier,
    rate_limit_key: String,
) -> Result<String, Error> {
    let job_service = ArticleJobService::new(state.clone());
    let prompt = normalize_create_prompt(&prompt)?;
    let permit = job_service.try_acquire_generation_slot("create")?;
    job_service.check_create_rate_limit(requester_tier, &rate_limit_key)?;

    let id = job_service.new_job_id();
    event!(Level::DEBUG, "Created id {}", &id);
    let return_id = id.clone();
    job_service
        .spawn_generation_job(id.clone(), permit, ArticleJobTrace::create(), async move {
            create_article(&state, id.clone(), prompt.clone(), author_email).await
        })
        .await;
    Ok(return_id)
}
