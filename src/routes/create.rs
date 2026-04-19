use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::{Form, Router};

use crate::app_state::AppState;
use crate::create as create_page;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/wait/{id}", get(get_wait))
        .route("/create", get(get_create).post(create_article))
}

async fn get_wait(wr: WibbleRequest, Path(id): Path<String>) -> Response {
    match create_page::wait(wr, &id).await {
        create_page::WaitResponse::Redirect(slug) => {
            Redirect::to(&format!("/content/{}", slug)).into_response()
        }
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
    let prompt = match create_page::normalize_create_prompt(&data.prompt) {
        Ok(prompt) => prompt,
        Err(Error::BadRequest(message)) => {
            return match create_page::render_create_page(&wr, data.prompt.trim(), Some(&message))
                .await
            {
                Ok(html) => (StatusCode::BAD_REQUEST, html).into_response(),
                Err(e) => e.into_response(),
            };
        }
        Err(e) => return e.into_response(),
    };

    match create_page::start_create_article(
        wr.state,
        prompt,
        author_email,
        wr.requester_tier,
        wr.rate_limit_key,
    )
    .await
    {
        Ok(id) => Redirect::to(&format!("/wait/{}", id)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    create_page::get_create(wr).await
}
