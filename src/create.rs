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
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;

pub async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    wr.template("create").await.render()
}

#[derive(Deserialize, Debug)]
pub struct PostCreateData {
    pub prompt: String,
}

async fn create_article(state: &AppState, id: String, instructions: String) -> Result<(), Error> {
    debug!("Generating article for instructions: {}", instructions);
    let model = state
        .llm
        .models
        .first()
        .ok_or_else(|| Error::Llm("No language model configured".to_string()))?;

    let use_examples_env = env::var("USE_EXAMPLES").unwrap_or("false".to_string());
    debug!("USE_EXAMPLES: {}", use_examples_env);

    let use_placeholders = env::var("USE_PLACEHOLDERS") == Ok("true".to_string());
    let can_use_examples = env::var("USE_EXAMPLES") == Ok("true".to_string());

    debug!("can_use_examples: {}", can_use_examples);
    let use_examples = can_use_examples;
    debug!("single attempt use_examples {}", use_examples);
    if use_placeholders {
        create_article_using_placeholders(state, id, instructions, model, use_examples).await
    } else {
        create_article_attempt(state, id, instructions, model).await
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
            let r = wr.template("wait").await.insert("id", id).render();
            match r {
                Ok(html) => WaitResponse::Html(html),
                Err(_) => WaitResponse::InternalError,
            }
        }
        _ => WaitResponse::NotFound,
    }
}

pub async fn start_create_article(state: AppState, prompt: String) -> Result<String, Error> {
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
    let id = Uuid::new_v4().to_string();
    event!(Level::DEBUG, "Created id {}", &id);
    let return_id = id.clone();
    let active_counter = state.active_article_generations.clone();
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
            let result = create_article(&state, id.clone(), prompt).await;
            let in_flight_after = active_counter
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);
            event!(
                Level::INFO,
                article_id = %id,
                in_flight = in_flight_after,
                "Finished article generation task"
            );
            result
        })
        .await;
    Ok(return_id)
}
