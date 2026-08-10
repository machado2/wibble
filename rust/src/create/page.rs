use axum::response::Html;

use crate::error::Error;
use crate::wibble_request::WibbleRequest;

use super::{CreateModeSelection, MAX_PROMPT_CHARS};

pub async fn render_create_page(
    wr: &WibbleRequest,
    prompt: &str,
    error_message: Option<&str>,
    _selected_mode: CreateModeSelection,
) -> Result<Html<String>, Error> {
    let mut template = wr.template("create").await;
    template
        .insert("title", "Create an article")
        .insert(
            "description",
            "Give Wibble News a prompt and generate an article.",
        )
        .insert("robots", "noindex,nofollow")
        .insert("prompt", &prompt)
        .insert("prompt_max_length", &MAX_PROMPT_CHARS);
    if let Some(error_message) = error_message {
        template.insert("error_message", error_message);
    }
    template.render()
}

pub async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    render_create_page(&wr, "", None, CreateModeSelection::Standard).await
}
