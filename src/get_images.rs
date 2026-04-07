use axum::extract::Query;
use axum::response::Html;
use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, QueryFilter, QueryOrder, QuerySelect};
use serde::Deserialize;

use crate::entities::content_image;
use crate::entities::prelude::*;
use crate::error::Error;
use crate::image_status::IMAGE_STATUS_COMPLETED;
use crate::wibble_request::WibbleRequest;

#[derive(Deserialize)]
pub struct GetImagesParams {
    after_id: Option<String>,
}

// Returns a list of image ids to show
pub async fn get_images(
    wr: WibbleRequest,
    Query(params): Query<GetImagesParams>,
) -> Result<Html<String>, Error> {
    let db = &wr.state.db;
    let has_next_page = params.after_id.is_some();
    let mut images = ContentImage::find()
        .filter(content_image::Column::Status.eq(IMAGE_STATUS_COMPLETED))
        .order_by_desc(content_image::Column::CreatedAt)
        .limit(100);
    if let Some(after_id) = params.after_id {
        images = images.filter(content_image::Column::Id.gt(after_id));
    }
    let images = images.all(db).await?;
    let ids: Vec<String> = images.into_iter().map(|i| i.id).collect();
    let last_id = ids.last().cloned().unwrap_or_default();
    let html = wr
        .template("images")
        .await
        .insert("items", &ids)
        .insert("last_id", &last_id)
        .insert("title", "Generated image gallery")
        .insert(
            "description",
            "A browsable gallery of AI-generated images used in Wibble stories.",
        )
        .insert(
            "robots",
            if has_next_page {
                "noindex,follow"
            } else {
                "noindex,nofollow"
            },
        )
        .render()?;
    Ok(html)
}
