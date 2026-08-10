use axum::extract::Path;
use axum::response::Html;
use axum::routing::get;
use axum::Router;

use crate::app_state::AppState;
use crate::content::GetContent;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub fn localized_router() -> Router<AppState> {
    Router::new().route("/content/{slug}", get(get_content))
}

async fn get_content(wr: WibbleRequest, Path(slug): Path<String>) -> Result<Html<String>, Error> {
    wr.get_content(&slug).await
}
