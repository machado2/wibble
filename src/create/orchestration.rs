use tracing::{event, Level};

use crate::app_state::AppState;
use crate::error::Error;
use crate::llm::article_generator::resolve_research_mode;
use crate::rate_limit::RequesterTier;
use crate::services::article_jobs::{ArticleJobRequest, ArticleJobService, ArticleJobTrace};

use super::{
    clarify::build_clarification_request, create_article, normalize_create_prompt,
    CreateModeSelection,
};

pub async fn start_create_article(
    state: AppState,
    prompt: String,
    author_email: Option<String>,
    requester_tier: RequesterTier,
    rate_limit_key: String,
    selected_mode: CreateModeSelection,
) -> Result<String, Error> {
    let job_service = ArticleJobService::new(state.clone());
    let prompt = normalize_create_prompt(&prompt)?;
    let research_mode = match selected_mode {
        CreateModeSelection::Auto => resolve_research_mode(&prompt, false, requester_tier)?,
        CreateModeSelection::Standard => None,
        CreateModeSelection::Research => resolve_research_mode(&prompt, true, requester_tier)?,
    };
    if research_mode.is_some() {
        job_service.check_research_rate_limit(requester_tier, &rate_limit_key)?;
    } else {
        job_service.check_create_rate_limit(requester_tier, &rate_limit_key)?;
    }
    let clarification = build_clarification_request(&prompt);
    let permit = if clarification.is_none() {
        Some(job_service.try_acquire_generation_slot("create")?)
    } else {
        None
    };

    let id = job_service.new_job_id();
    event!(Level::DEBUG, "Created id {}", &id);
    let return_id = id.clone();
    job_service
        .create_job(
            id.clone(),
            ArticleJobRequest::create(
                prompt.clone(),
                author_email.clone(),
                requester_tier,
                rate_limit_key,
                research_mode,
            ),
        )
        .await?;
    if let Some(clarification) = clarification {
        let payload = serde_json::to_string(&clarification)
            .map_err(|e| Error::Llm(format!("Failed to encode clarification request: {}", e)))?;
        job_service.request_clarification(&id, payload).await?;
        return Ok(return_id);
    }
    job_service
        .spawn_generation_job(
            id.clone(),
            permit.unwrap(),
            ArticleJobTrace::create(research_mode),
            async move {
                create_article(
                    &state,
                    id.clone(),
                    prompt.clone(),
                    author_email,
                    research_mode,
                )
                .await
            },
        )
        .await;
    Ok(return_id)
}
