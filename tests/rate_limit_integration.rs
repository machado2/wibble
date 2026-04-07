use std::env;

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt; // for `oneshot`
use wibble::rate_limit::{ArticleRateLimit, RateLimitState};

async fn limited_create(State(state): State<RateLimitState>) -> StatusCode {
    match state.check_article_generation_limit() {
        Ok(()) => StatusCode::OK,
        Err(ArticleRateLimit::Hourly | ArticleRateLimit::Daily) => StatusCode::TOO_MANY_REQUESTS,
    }
}

#[tokio::test]
async fn article_generation_limit_allows_configured_burst_and_blocks_next() {
    let hourly_max: u32 = env::var("MAX_ARTICLES_PER_HOUR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let hourly_burst: u32 = env::var("MAX_ARTICLES_BURST_PER_HOUR")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(|v: u32| v.clamp(1, hourly_max))
        .unwrap_or(hourly_max);
    let daily_max: u32 = env::var("MAX_ARTICLES_PER_DAY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let daily_burst: u32 = env::var("MAX_ARTICLES_BURST_PER_DAY")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(|v: u32| v.clamp(1, daily_max))
        .unwrap_or(daily_max);
    let allowed = hourly_burst.min(daily_burst);

    let state = RateLimitState::new();
    let app = Router::new()
        .route("/create", post(limited_create))
        .with_state(state.clone());

    for i in 0..allowed {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/create")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");
        assert_eq!(response.status(), StatusCode::OK, "failed at {}", i);
    }

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/create")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}
