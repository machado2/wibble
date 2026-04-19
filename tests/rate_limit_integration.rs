use std::env;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, Request, StatusCode},
    routing::post,
    Router,
};
use tower::ServiceExt; // for `oneshot`
use wibble::rate_limit::{ArticleRateLimit, RateLimitState, RequesterTier};

async fn limited_create(State(state): State<RateLimitState>, headers: HeaderMap) -> StatusCode {
    let key = headers
        .get("x-rate-key")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("anon-key-a");
    match state.check_article_generation_limit(RequesterTier::Anonymous, key) {
        Ok(()) => StatusCode::OK,
        Err(ArticleRateLimit::Hourly | ArticleRateLimit::Daily) => StatusCode::TOO_MANY_REQUESTS,
    }
}

#[tokio::test]
async fn article_generation_limit_is_keyed_by_request_identity() {
    let hourly_max: u32 = env::var("MAX_ARTICLES_PER_HOUR_ANON")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            env::var("MAX_ARTICLES_PER_HOUR")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(10);
    let hourly_burst: u32 = env::var("MAX_ARTICLES_BURST_PER_HOUR_ANON")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            env::var("MAX_ARTICLES_BURST_PER_HOUR")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .map(|v: u32| v.clamp(1, hourly_max))
        .unwrap_or(hourly_max);
    let daily_max: u32 = env::var("MAX_ARTICLES_PER_DAY_ANON")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            env::var("MAX_ARTICLES_PER_DAY")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(20);
    let daily_burst: u32 = env::var("MAX_ARTICLES_BURST_PER_DAY_ANON")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            env::var("MAX_ARTICLES_BURST_PER_DAY")
                .ok()
                .and_then(|s| s.parse().ok())
        })
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
                    .header("x-rate-key", "anon-key-a")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");
        assert_eq!(response.status(), StatusCode::OK, "failed at {}", i);
    }

    let blocked_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/create")
                .header("x-rate-key", "anon-key-a")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(blocked_response.status(), StatusCode::TOO_MANY_REQUESTS);

    let different_key_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/create")
                .header("x-rate-key", "anon-key-b")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");
    assert_eq!(different_key_response.status(), StatusCode::OK);
}
