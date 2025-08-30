use std::env;
use std::net::Ipv4Addr;

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{middleware, serve, Form, Router};
use dotenvy::dotenv;
use rand::Rng;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

use crate::app_state::AppState;
use crate::content::GetContent;
use crate::create::{start_create_article, wait, PostCreateData, WaitResponse};
use crate::error::Error;
use crate::image_info::get_image_info_handler;
use crate::newslist::{ContentListParams, NewsList};
use crate::wibble_request::WibbleRequest;
use serde::Deserialize;

mod newslist;

mod app_state;
mod content;
mod create;

mod entities;
mod error;
mod get_images;
mod image;
mod image_generator;
mod image_info;
mod llm;
mod repository;
mod s3;
mod tasklist;
mod wibble_request;

// #[debug_handler(state = AppState)]
async fn get_index(
    wr: WibbleRequest,
    Form(data): Form<ContentListParams>,
) -> Result<Html<String>, Error> {
    wr.news_list(data).await
}

#[derive(Deserialize)]
struct ContentQuery {
    source: Option<String>,
}

async fn get_content(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Query(query): Query<ContentQuery>,
) -> Result<Html<String>, Error> {
    wr.get_content(&slug, query.source.as_deref()).await
}

async fn get_image(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, StatusCode> {
    let db = &state.db;
    let img = image::get_image(db, &id)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Response::builder()
        .header("Content-Type", "image/jpeg")
        .body(Body::from(Bytes::from(img)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_wait(wr: WibbleRequest, Path(id): Path<String>) -> Response {
    match wait(wr, &id).await {
        WaitResponse::Redirect(slug) => {
            let url = format!("/content/{}", slug);
            Redirect::to(&url).into_response()
        }
        WaitResponse::Html(html) => html.into_response(),
        WaitResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        WaitResponse::InternalError => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn create_en(
    State(state): State<AppState>,
    Form(data): Form<PostCreateData>,
) -> impl IntoResponse {
    let id = tokio::task::spawn(start_create_article(state, data.prompt))
        .await
        .unwrap();

    // let id = start_create_article(state, data.prompt).await;
    Redirect::to(&format!("/wait/{}", id)).into_response()
}

async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    create::get_create(wr).await
}

async fn handle_error(
    wr: WibbleRequest,
    req: axum::http::Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let response = next.run(req).await;
    let status_code = response.status();
    match status_code {
        StatusCode::INTERNAL_SERVER_ERROR => {
            let image_url = format!("/error{}.jpg", rand::rng().random_range(1..=8));
            wr.template("error")
                .await
                .insert("image_url", &image_url)
                .insert(
                    "error_message",
                    "Oops! Something went wrong. Please try again later.",
                )
                .render()
                .into_response()
        }
        StatusCode::NOT_FOUND => {
            let image_url = format!("/notfound{}.jpg", rand::rng().random_range(1..=4));
            wr.template("error")
                .await
                .insert("image_url", &image_url)
                .insert(
                    "error_message",
                    "The page you are looking for does not exist.",
                )
                .render()
                .into_response()
        }
        _ => response,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let port: u16 = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse()
        .unwrap();
    let serve_dir = ServeDir::new("static");
    let state = AppState::init().await;
    let app = Router::new()
        .route("/", get(get_index))
        .route("/image/{id}", get(get_image))
        .route("/image_info/{id}", get(get_image_info_handler))
        .route("/content/{slug}", get(get_content))
        .route("/wait/{id}", get(get_wait))
        .route("/create", post(create_en).get(get_create))
        .route("/images", get(get_images::get_images))
        .fallback_service(serve_dir)
        .layer(middleware::from_fn_with_state(state.clone(), handle_error))
        .with_state(state);
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .unwrap();
    serve(listener, app.into_make_service()).await.unwrap();
}
