use axum::{middleware, Router};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use crate::app_state::AppState;
use crate::rate_limit::rate_limit_middleware;
use crate::routes::{admin, auth, content, create, edit, legacy, public};

pub fn build_router(state: AppState) -> Router {
    let serve_dir = ServeDir::new("static");
    let localized_app = Router::new()
        .merge(public::localized_router())
        .merge(create::localized_router())
        .merge(content::localized_router())
        .merge(edit::localized_router())
        .merge(admin::localized_router())
        .merge(auth::localized_router())
        .layer(middleware::from_fn(legacy::persist_site_language_cookie));

    Router::new()
        .merge(public::global_router())
        .merge(auth::global_callback_router())
        .merge(legacy::router())
        .nest("/en", localized_app.clone())
        .nest("/pt", localized_app)
        .fallback_service(serve_dir)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            state.rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            public::handle_error,
        ))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use http::{Request, StatusCode};
    use tower::ServiceExt;

    use crate::server::build_router;
    use crate::test_support::TestContext;

    #[tokio::test]
    async fn app_router_builds_without_panicking() {
        let ctx = TestContext::new().await;
        let _ = build_router(ctx.state.clone());
    }

    #[tokio::test]
    async fn localized_root_routes_respond_successfully() {
        let ctx = TestContext::new().await;
        let app = build_router(ctx.state.clone());

        let response = app
            .clone()
            .oneshot(Request::builder().uri("/pt").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(Request::builder().uri("/pt/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn legacy_content_root_redirects_to_localized_home() {
        let ctx = TestContext::new().await;
        let app = build_router(ctx.state.clone());

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/content/")
                    .header(http::header::ACCEPT_LANGUAGE, "pt-BR,pt;q=0.9,en;q=0.8")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(http::header::LOCATION).unwrap(),
            "/pt/"
        );
    }
}
