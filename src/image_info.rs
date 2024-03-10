use axum::extract::Path;
use axum::response::Html;
use sea_orm::EntityTrait;

use crate::entities::prelude::*;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub async fn get_image_info_handler(
    wr: WibbleRequest,
    Path(id): Path<String>,
) -> Result<Html<String>, Error> {
    let db = &wr.state.db;
    let (info, article) = ContentImage::find_by_id(id.clone())
        .find_also_related(Content)
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading image info: {}", e)))?
        .ok_or(Error::NotFound)?;
    let (slug, article_title) = if let Some(article) = article {
        (article.slug, article.title)
    } else {
        return Err(Error::NotFound);
    };
    wr.template("image_info")
        .await
        .insert("image_id", &id)
        .insert("title", &info.alt_text)
        .insert("created_at", &info.created_at.format("%F").to_string())
        .insert("slug", &slug)
        .insert("content_title", &article_title)
        .insert("prompt", &info.prompt)
        .render()
}
