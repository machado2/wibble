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
async fn global_limit_allows_100_and_blocks_101st() {
    // Build app with only the rate limit middleware and a dummy /create handler
    let state = RateLimitState::new();
    let app = Router::new()
        .route("/create", post(|| async { "ok" }))
        .layer(middleware::from_fn_with_state(state.clone(), rate_limit_middleware));

    // First 100 should pass
    for i in 0..100u32 {
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

    // 101st should be rate limited
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

