#![allow(clippy::blocks_in_conditions)]

use std::env;
use std::sync::atomic::Ordering;

use axum::response::Html;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait};
use serde::{Deserialize, Serialize};
use tracing::{debug, event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::entities::prelude::*;
use crate::entities::{content, content_image};
use crate::error::Error;
use crate::image_status::{
    IMAGE_STATUS_COMPLETED, IMAGE_STATUS_FAILED, IMAGE_STATUS_PENDING, IMAGE_STATUS_PROCESSING,
};
use crate::llm::article_generator::{create_article_attempt, create_article_using_placeholders};
use crate::rate_limit::ArticleRateLimit;
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;

const MAX_PROMPT_CHARS: usize = 600;

#[derive(Serialize)]
struct PromptPreset {
    label: &'static str,
    prompt: &'static str,
}

#[derive(Serialize)]
struct WaitSummary {
    article_title: Option<String>,
    slug: Option<String>,
    stage_title: String,
    stage_description: String,
    image_total: usize,
    image_completed: usize,
    image_processing: usize,
    image_failed: usize,
}

fn create_prompt_presets() -> [PromptPreset; 4] {
    [
        PromptPreset {
            label: "Tech Meltdown",
            prompt: "A major tech company accidentally replaces its CEO with an overly enthusiastic AI intern during a product launch.",
        },
        PromptPreset {
            label: "Town Hall",
            prompt: "A sleepy coastal town becomes obsessed with electing a seagull as mayor after it solves one local problem too many.",
        },
        PromptPreset {
            label: "Sports Chaos",
            prompt: "A football match spirals into absurdity when every coach starts using motivational corporate jargon instead of tactics.",
        },
        PromptPreset {
            label: "Science Desk",
            prompt: "Scientists announce a world-changing discovery, but the lab notes read like a group chat that got wildly out of hand.",
        },
    ]
}

pub fn normalize_create_prompt(raw: &str) -> Result<String, Error> {
    let prompt = raw.trim();
    if prompt.is_empty() {
        return Err(Error::BadRequest(
            "Add a prompt before generating an article.".to_string(),
        ));
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(Error::BadRequest(format!(
            "Prompt is too long. Keep it under {} characters.",
            MAX_PROMPT_CHARS
        )));
    }
    Ok(prompt.to_string())
}

pub async fn render_create_page(
    wr: &WibbleRequest,
    prompt: &str,
    error_message: Option<&str>,
) -> Result<Html<String>, Error> {
    let presets = create_prompt_presets();
    let mut template = wr.template("create").await;
    template
        .insert("title", "Create a new article")
        .insert(
            "description",
            "Submit a prompt and let The Wibble generate a new satirical article.",
        )
        .insert("robots", "noindex,nofollow")
        .insert("prompt", &prompt)
        .insert("prompt_max_length", &MAX_PROMPT_CHARS)
        .insert("prompt_presets", &presets);
    if let Some(error_message) = error_message {
        template.insert("error_message", error_message);
    }
    template.render()
}

pub async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    render_create_page(&wr, "", None).await
}

#[derive(Deserialize, Debug)]
pub struct PostCreateData {
    pub prompt: String,
}

async fn create_article(
    state: &AppState,
    id: String,
    instructions: String,
    author_email: Option<String>,
) -> Result<(), Error> {
    debug!("Generating article for instructions: {}", instructions);
    let model = state
        .llm
        .models
        .first()
        .ok_or_else(|| Error::Llm("No language model configured".to_string()))?;

    let use_examples_env = env::var("USE_EXAMPLES").unwrap_or("false".to_string());
    debug!("USE_EXAMPLES: {}", use_examples_env);

    let use_placeholders = env::var("USE_PLACEHOLDERS")
        .ok()
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "no" | "off")
        })
        .unwrap_or(true);
    let can_use_examples = env::var("USE_EXAMPLES") == Ok("true".to_string());

    debug!("use_placeholders: {}", use_placeholders);
    debug!("can_use_examples: {}", can_use_examples);
    let use_examples = can_use_examples;
    debug!("single attempt use_examples {}", use_examples);
    if use_placeholders {
        create_article_using_placeholders(
            state,
            id,
            instructions,
            model,
            use_examples,
            author_email,
        )
        .await
    } else {
        create_article_attempt(state, id, instructions, model, author_email).await
    }
}

#[allow(clippy::large_enum_variant)]
pub enum WaitResponse {
    Redirect(String),
    Html(Html<String>),
    InternalError,
    NotFound,
}

async fn build_wait_summary(state: &AppState, id: &str) -> Result<WaitSummary, Error> {
    let article = Content::find()
        .filter(content::Column::Id.eq(id))
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article wait state: {}", e)))?;

    if let Some(article) = article {
        let images = ContentImage::find()
            .filter(content_image::Column::ContentId.eq(article.id.clone()))
            .all(&state.db)
            .await
            .map_err(|e| Error::Database(format!("Error loading image wait state: {}", e)))?;
        let image_total = images.len();
        let image_completed = images
            .iter()
            .filter(|img| img.status == IMAGE_STATUS_COMPLETED)
            .count();
        let image_processing = images
            .iter()
            .filter(|img| {
                img.status == IMAGE_STATUS_PROCESSING || img.status == IMAGE_STATUS_PENDING
            })
            .count();
        let image_failed = images
            .iter()
            .filter(|img| img.status == IMAGE_STATUS_FAILED)
            .count();
        let (stage_title, stage_description) = if image_total == 0 && article.markdown.is_none() {
            (
                "Drafting the story".to_string(),
                "The headline, angle, and article body are still being assembled.".to_string(),
            )
        } else if image_total == 0 {
            (
                "Preparing the article".to_string(),
                "The story draft is ready and the page is being finalized.".to_string(),
            )
        } else if image_processing > 0 {
            (
                "Rendering illustrations".to_string(),
                "The story is ready and the image queue is actively rendering art.".to_string(),
            )
        } else if image_failed > 0 && image_completed < image_total {
            (
                "Recovering the image set".to_string(),
                "Some illustrations failed and the article is waiting on the remaining results."
                    .to_string(),
            )
        } else {
            (
                "Finalizing the article".to_string(),
                "The draft is complete and the page is about to go live.".to_string(),
            )
        };

        Ok(WaitSummary {
            article_title: Some(article.title),
            slug: Some(article.slug),
            stage_title,
            stage_description,
            image_total,
            image_completed,
            image_processing,
            image_failed,
        })
    } else {
        Ok(WaitSummary {
            article_title: None,
            slug: None,
            stage_title: "Drafting the story".to_string(),
            stage_description:
                "The prompt is in the queue and the article body is still being written."
                    .to_string(),
            image_total: 0,
            image_completed: 0,
            image_processing: 0,
            image_failed: 0,
        })
    }
}

pub async fn render_wait_page(wr: &WibbleRequest, id: &str) -> Result<Html<String>, Error> {
    let wait_summary = build_wait_summary(&wr.state, id).await?;
    wr.template("wait")
        .await
        .insert("id", id)
        .insert("title", "Generating article")
        .insert(
            "description",
            "The article is still being generated and this page auto-refreshes.",
        )
        .insert("robots", "noindex,nofollow")
        .insert("wait_summary", &wait_summary)
        .render()
}

pub async fn wait(wr: WibbleRequest, id: &str) -> WaitResponse {
    let task = wr.state.task_list.get(id).await;
    match task {
        Ok(TaskResult::Success) => {
            let c = Content::find()
                .filter(content::Column::Id.eq(id))
                .one(&wr.state.db)
                .await;
            match c {
                Ok(Some(c)) => WaitResponse::Redirect(c.slug),
                Ok(None) => WaitResponse::NotFound,
                Err(_) => WaitResponse::InternalError,
            }
        }
        Ok(TaskResult::Error) => WaitResponse::InternalError,
        Ok(TaskResult::Processing) => match render_wait_page(&wr, id).await {
            Ok(html) => WaitResponse::Html(html),
            Err(_) => WaitResponse::InternalError,
        },
        _ => WaitResponse::NotFound,
    }
}

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

fn recover_prompt_from_slug(slug: &str) -> String {
    let topic = slug
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if topic.is_empty() {
        slug.to_string()
    } else {
        topic
    }
}

pub async fn start_recover_article_for_slug(
    state: AppState,
    slug: String,
) -> Result<Option<String>, Error> {
    let slug = slug.trim().to_string();
    if slug.is_empty() {
        return Ok(None);
    }

    let permit = state
        .article_generation_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            event!(
                Level::WARN,
                "Rejected dead-link recovery due to concurrency limit (MAX_CONCURRENT_ARTICLE_GENERATIONS reached)",
            );
            Error::RateLimited
        })?;

    let model = state
        .llm
        .models
        .first()
        .ok_or_else(|| Error::Llm("No language model configured".to_string()))?
        .to_string();

    let id = Uuid::new_v4().to_string();
    let return_id = id.clone();
    let prompt = recover_prompt_from_slug(&slug);
    let now = chrono::Utc::now().naive_local();
    let placeholder = content::Model {
        id: id.clone(),
        slug: slug.clone(),
        content: None,
        created_at: now,
        generating: true,
        generation_started_at: Some(now),
        generation_finished_at: None,
        flagged: false,
        model: model.clone(),
        prompt_version: 0,
        fail_count: 0,
        description: format!("Recovered dead link: /content/{}", slug),
        image_id: None,
        title: slug.replace('-', " "),
        user_input: prompt.clone(),
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: None,
        converted: false,
        longview_count: 0,
        impression_count: 0,
        click_count: 0,
        author_email: None,
        published: true,
        recovered_from_dead_link: true,
    };
    let insert_result = Content::insert(content::ActiveModel::from(placeholder))
        .exec(&state.db)
        .await;
    if insert_result.is_err() {
        event!(
            Level::WARN,
            slug = %slug,
            "Dead-link recovery placeholder insert failed; likely slug already exists"
        );
        return Ok(None);
    }

    if !state.try_take_dead_link_recovery_slot().await {
        event!(
            Level::WARN,
            slug = %slug,
            max_per_day = state.dead_link_recovery_max_per_day,
            "Dead-link recovery skipped due to daily limit"
        );
        let _ = Content::delete_by_id(id).exec(&state.db).await;
        return Ok(None);
    }

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
                recovery_slug = %slug,
                in_flight,
                "Started dead-link recovery generation task"
            );
            let result = create_article(&state, id.clone(), prompt, None).await;
            let in_flight_after = active_counter
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);
            if result.is_err() {
                let _ = Content::delete_by_id(id.clone()).exec(&state.db).await;
            }
            event!(
                Level::INFO,
                article_id = %id,
                recovery_slug = %slug,
                in_flight = in_flight_after,
                "Finished dead-link recovery generation task"
            );
            state.mark_generation_finished(&id).await;
            result
        })
        .await;

    Ok(Some(return_id))
}

#[cfg(test)]
mod tests {
    use super::normalize_create_prompt;

    #[test]
    fn create_prompt_validation_trims_and_rejects_empty_input() {
        assert_eq!(
            normalize_create_prompt("  hello wobble  ").unwrap(),
            "hello wobble"
        );
        assert!(normalize_create_prompt("   ").is_err());
    }

    #[test]
    fn create_prompt_validation_rejects_overly_long_input() {
        let prompt = "a".repeat(601);
        assert!(normalize_create_prompt(&prompt).is_err());
    }
}
