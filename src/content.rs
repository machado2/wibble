use axum::response::Html;

use crate::error::Error;
use crate::wibble_request::WibbleRequest;

mod page;
mod policy;
mod query;
mod render;

pub use policy::can_view_article;
pub use query::{find_article_by_slug, require_article_by_slug};

#[allow(async_fn_in_trait)]
pub trait GetContent {
    async fn get_content(&self, slug: &str) -> Result<Html<String>, Error>;
}

impl GetContent for WibbleRequest {
    async fn get_content(&self, slug: &str) -> Result<Html<String>, Error> {
        page::render_content_page(self, slug).await
    }
}
