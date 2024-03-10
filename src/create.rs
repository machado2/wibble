#![allow(clippy::blocks_in_conditions)]

use std::env;

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
    let models = &state.llm.models;
    let mut attempts = 0;

    let use_examples_env = env::var("USE_EXAMPLES").unwrap();
    debug!("USE_EXAMPLES: {}", use_examples_env);

    let use_placeholders = env::var("USE_PLACEHOLDERS") == Ok("true".to_string());
    let can_use_examples = env::var("USE_EXAMPLES") == Ok("true".to_string());

    debug!("can_use_examples: {}", can_use_examples);

    loop {
        let model = &models[attempts % models.len()];
        attempts += 1;
        let use_examples = can_use_examples || attempts > 1;
        debug!("attempt {} use_examples {}", attempts, use_examples);
        let res = if use_placeholders {
            create_article_using_placeholders(
                state,
                id.clone(),
                instructions.clone(),
                model,
                use_examples,
            )
            .await
        } else {
            create_article_attempt(state, id.clone(), instructions.clone(), model).await
        };
        match res {
            Ok(article) => {
                return Ok(article);
            }
            Err(e) => {
                event!(Level::DEBUG, "Attempt {} failed, error: {}", attempts, e,);
                if attempts >= 3 {
                    event!(
                        Level::ERROR,
                        "Failed to generate article after 3 attempts: {}",
                        e
                    );
                    return Err(e);
                }
            }
        }
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

pub async fn start_create_article(state: AppState, prompt: String) -> String {
    let id = Uuid::new_v4().to_string();
    event!(Level::DEBUG, "Created id {}", &id);
    let return_id = id.clone();
    state
        .task_list
        .clone()
        .spawn_task(id.clone(), async move {
            create_article(&state, id.clone(), prompt).await
        })
        .await;
    return_id
}
