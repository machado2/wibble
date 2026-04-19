use axum::body::{Body, Bytes};
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use axum::Router;
use rand::Rng;

use crate::app_state::AppState;
use crate::error::Error;
use crate::newslist::{ContentListParams, NewsList};
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_index))
        .route("/sitemap.xml", get(crate::sitemap::get_sitemap))
        .route("/robots.txt", get(crate::sitemap::get_robots_txt))
        .route("/image/{id}", get(get_image))
        .route(
            "/image_info/{id}",
            get(crate::image_info::get_image_info_handler),
        )
        .route("/images", get(crate::get_images::get_images))
}

async fn get_index(
    wr: WibbleRequest,
    Query(data): Query<ContentListParams>,
) -> Result<Html<String>, Error> {
    wr.news_list(data).await
}

async fn get_image(wr: WibbleRequest, Path(id): Path<String>) -> Result<Response, StatusCode> {
    let img = crate::image::get_image(&wr.state, &id, wr.auth_user.as_ref())
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Response::builder()
        .header("Content-Type", img.content_type)
        .header("Cache-Control", img.cache_control)
        .body(Body::from(Bytes::from(img.bytes)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn handle_error(
    wr: WibbleRequest,
    req: axum::http::Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let response = next.run(req).await;

    match response.status() {
        StatusCode::INTERNAL_SERVER_ERROR => {
            let image_url = format!("/error{}.jpg", rand::rng().random_range(1..=8));
            wr.template("error")
                .await
                .insert("title", "Server error")
                .insert(
                    "description",
                    "An unexpected server error occurred while loading this page.",
                )
                .insert("robots", "noindex,nofollow")
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
                .insert("title", "Page not found")
                .insert("description", "The requested page could not be found.")
                .insert("robots", "noindex,nofollow")
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
