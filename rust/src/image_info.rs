use axum::extract::Path;
use axum::response::Html;
use sea_orm::EntityTrait;

use crate::content::can_view_article;
use crate::entities::prelude::*;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub async fn get_image_info_handler(
    wr: WibbleRequest,
    Path(id): Path<String>,
) -> Result<Html<String>, Error> {
    let text = wr.site_text();
    let db = &wr.state.db;
    let (info, article) = ContentImage::find_by_id(id.clone())
        .find_also_related(Content)
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading image info: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Image {} not found", id))))?;
    let (slug, article_title) = if let Some(article) = article {
        if !can_view_article(wr.auth_user.as_ref(), &article) {
            return Err(Error::NotFound(Some(format!("Image {} not found", id))));
        }
        (article.slug, article.title)
    } else {
        return Err(Error::NotFound(Some(format!(
            "Article for image {} not found",
            id
        ))));
    };

    wr.template("image_info")
        .await
        .insert("image_id", &id)
        .insert("title", &info.alt_text)
        .insert("created_at", &info.created_at.format("%F").to_string())
        .insert("slug", &slug)
        .insert("content_title", &article_title)
        .insert("prompt", &info.prompt)
        .insert("status", &info.status)
        .insert("last_error", &info.last_error)
        .insert("model", &info.model)
        .insert("description", text.image_info_description())
        .insert("robots", "noindex,nofollow")
        .render()
}
