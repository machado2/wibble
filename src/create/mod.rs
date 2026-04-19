#![allow(clippy::blocks_in_conditions)]

mod input;
mod orchestration;
mod page;
mod recovery;
mod wait;

use std::env;

use tracing::debug;

use crate::app_state::AppState;
use crate::error::Error;
use crate::llm::article_generator::{create_article_attempt, create_article_using_placeholders};

const MAX_PROMPT_CHARS: usize = 600;

pub use input::{normalize_create_prompt, PostCreateData};
pub use orchestration::start_create_article;
pub use page::{get_create, render_create_page};
pub use recovery::start_recover_article_for_slug;
pub use wait::{render_wait_page, wait, WaitResponse};

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
