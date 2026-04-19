use std::env;
use std::net::Ipv4Addr;

use axum::{middleware, serve, Router};
use dotenvy::dotenv;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

use wibble::app_state::AppState;
use wibble::rate_limit::rate_limit_middleware;
use wibble::routes::{admin, auth, content, create, edit, public};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let port: u16 = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse()
        .unwrap();
    let serve_dir = ServeDir::new("static");
    let state = AppState::init()
        .await
        .unwrap_or_else(|e| panic!("Failed to initialize application state: {}", e));

    let app = Router::new()
        .merge(public::router())
        .merge(create::router())
        .merge(content::router())
        .merge(edit::router())
        .merge(admin::router())
        .merge(auth::router())
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
        .with_state(state);

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .unwrap();
    serve(listener, app.into_make_service()).await.unwrap();
}
