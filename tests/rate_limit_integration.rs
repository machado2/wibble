use std::env;

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware,
    routing::post,
    Router,
};
use tower::ServiceExt; // for `oneshot`
use wibble::rate_limit::{rate_limit_middleware, RateLimitState};

#[tokio::test]
async fn hourly_limit_allows_max_and_blocks_next() {
    let max: u32 = env::var("MAX_ARTICLES_PER_HOUR")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20);
    let burst: u32 = env::var("MAX_ARTICLES_BURST_PER_HOUR")
        .ok()
        .and_then(|s| s.parse().ok())
        .map(|v: u32| v.clamp(1, max))
        .unwrap_or(1);

    let state = RateLimitState::new();
    let app = Router::new()
        .route("/create", post(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));

    for i in 0..burst {
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

    // Next should be rate limited
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
