use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Form, Router};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::create as create_page;
use crate::create::clarify::normalize_clarification_answer;
use crate::error::Error;
use crate::services::article_jobs::{spawn_due_article_jobs, ArticleJobService};
use crate::services::site_paths::localized_path;
use crate::wibble_request::WibbleRequest;

pub fn localized_router() -> Router<AppState> {
    Router::new()
        .route("/wait/{id}", get(get_wait))
        .route(
            "/wait/{id}/clarify",
            axum::routing::post(post_wait_clarification),
        )
        .route("/create", get(get_create).post(create_article))
}

#[derive(Deserialize)]
struct WaitClarificationData {
    answer: String,
}

async fn get_wait(wr: WibbleRequest, Path(id): Path<String>) -> Response {
    let site_language = wr.site_language;
    match create_page::wait(wr, &id).await {
        create_page::WaitResponse::Redirect(slug) => Redirect::to(&localized_path(
            site_language,
            &format!("/content/{}", slug),
        ))
        .into_response(),
        create_page::WaitResponse::Html(html) => html.into_response(),
        create_page::WaitResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        create_page::WaitResponse::InternalError => {
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn create_article(
    wr: WibbleRequest,
    Form(data): Form<create_page::PostCreateData>,
) -> impl IntoResponse {
    let author_email = wr.auth_user.as_ref().map(|u| u.email.clone());
    let selected_mode = match create_page::normalize_create_mode(data.mode.as_deref()) {
        Ok(mode) => mode,
        Err(Error::BadRequest(message)) => {
            return match create_page::render_create_page(
                &wr,
                data.prompt.trim(),
                Some(&message),
                create_page::CreateModeSelection::Auto,
            )
            .await
            {
                Ok(html) => (StatusCode::BAD_REQUEST, html).into_response(),
                Err(e) => e.into_response(),
            };
        }
        Err(e) => return e.into_response(),
    };
    let prompt = match create_page::normalize_create_prompt(&data.prompt) {
        Ok(prompt) => prompt,
        Err(Error::BadRequest(message)) => {
            return match create_page::render_create_page(
                &wr,
                data.prompt.trim(),
                Some(&message),
                selected_mode,
            )
            .await
            {
                Ok(html) => (StatusCode::BAD_REQUEST, html).into_response(),
                Err(e) => e.into_response(),
            };
        }
        Err(e) => return e.into_response(),
    };

    match create_page::start_create_article(
        wr.state.clone(),
        prompt.clone(),
        author_email,
        wr.requester_tier,
        wr.rate_limit_key.clone(),
        selected_mode,
    )
    .await
    {
        Ok(id) => Redirect::to(&wr.localized_path(&format!("/wait/{}", id))).into_response(),
        Err(Error::BadRequest(message)) | Err(Error::Auth(message)) => {
            match create_page::render_create_page(&wr, &prompt, Some(&message), selected_mode).await
            {
                Ok(html) => (StatusCode::BAD_REQUEST, html).into_response(),
                Err(e) => e.into_response(),
            }
        }
        Err(e) => e.into_response(),
    }
}

async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    create_page::get_create(wr).await
}

async fn post_wait_clarification(
    wr: WibbleRequest,
    Path(id): Path<String>,
    Form(data): Form<WaitClarificationData>,
) -> Response {
    let answer = match normalize_clarification_answer(&data.answer) {
        Ok(answer) => answer,
        Err(e) => return e.into_response(),
    };

    let service = ArticleJobService::new(wr.state.clone());
    match service.submit_clarification_answer(&id, &answer).await {
        Ok(Some(_)) => {
            spawn_due_article_jobs(wr.state.clone()).await;
            Redirect::to(&wr.localized_path(&format!("/wait/{}", id))).into_response()
        }
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(e) => e.into_response(),
    }
}
