#![allow(clippy::blocks_in_conditions)]

use std::env;
use std::sync::atomic::Ordering;

use axum::response::Html;
use sea_orm::QueryFilter;
use sea_orm::{ColumnTrait, EntityTrait};
use serde::Deserialize;
use tracing::{debug, event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::entities::content;
use crate::entities::prelude::*;
use crate::error::Error;
use crate::llm::article_generator::{create_article_attempt, create_article_using_placeholders};
use crate::rate_limit::ArticleRateLimit;
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;

pub async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    wr.template("create")
        .await
        .insert("title", "Create a new article")
        .insert(
            "description",
            "Submit a prompt and let The Wibble generate a new satirical article.",
        )
        .insert("robots", "noindex,nofollow")
        .render()
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
        Ok(TaskResult::Processing) => {
            let r = wr
                .template("wait")
                .await
                .insert("id", id)
                .insert("title", "Generating article")
                .insert(
                    "description",
                    "The article is still being generated and this page auto-refreshes.",
                )
                .insert("robots", "noindex,nofollow")
                .render();
            match r {
                Ok(html) => WaitResponse::Html(html),
                Err(_) => WaitResponse::InternalError,
            }
        }
        _ => WaitResponse::NotFound,
    }
}

pub async fn start_create_article(
    state: AppState,
    prompt: String,
    author_email: Option<String>,
) -> Result<String, Error> {
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
