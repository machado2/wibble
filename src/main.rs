use std::env;
use std::net::Ipv4Addr;

use axum::serve;
use dotenvy::dotenv;
use tokio::net::TcpListener;

use wibble::app_state::AppState;
use wibble::server::build_router;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let port: u16 = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse()
        .unwrap();
    let state = AppState::init()
        .await
        .unwrap_or_else(|e| panic!("Failed to initialize application state: {}", e));
    let app = build_router(state);

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .unwrap();
    serve(listener, app.into_make_service()).await.unwrap();
}
