use axum::body::Body;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderValue;
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;
use axum::Router;

use crate::app_state::AppState;
use crate::services::site_paths::{
    detect_site_language_from_path, find_supported_site_language, site_language_cookie_header,
};
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(redirect_home))
        .route("/create", get(redirect_same_path))
        .route("/wait/{*rest}", get(redirect_same_path))
        .route("/content", get(redirect_content_root))
        .route("/content/", get(redirect_content_root))
        .route("/content/{*rest}", get(redirect_same_path))
        .route("/images", get(redirect_same_path))
        .route("/image_info/{id}", get(redirect_same_path))
        .route("/admin/{*rest}", get(redirect_same_path))
        .route("/login", get(redirect_same_path))
        .route("/logout", get(redirect_same_path))
}

async fn redirect_home(wr: WibbleRequest) -> Redirect {
    Redirect::to(&wr.localized_root_path())
}

async fn redirect_content_root(wr: WibbleRequest) -> Redirect {
    Redirect::to(&wr.localized_root_path())
}

async fn redirect_same_path(wr: WibbleRequest) -> Redirect {
    Redirect::to(&wr.localized_request_path())
}

pub async fn persist_site_language_cookie(
    req: axum::http::Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let explicit_language = detect_site_language_from_path(req.uri().path()).or_else(|| {
        req.uri()
            .path()
            .trim_start_matches('/')
            .split('/')
            .next()
            .and_then(find_supported_site_language)
    });
    let mut response = next.run(req).await;
    if let Some(language) = explicit_language {
        let cookie = site_language_cookie_header(language);
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    response
}
