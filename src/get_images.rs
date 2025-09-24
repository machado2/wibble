use axum::extract::Query;
use axum::response::Html;
use sea_orm::prelude::*;
use sea_orm::QuerySelect;
use serde::Deserialize;

use crate::entities::content_image;
use crate::entities::prelude::*;
use crate::error::Error;
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
    let mut images = ContentImage::find().limit(100);
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
        .render()?;
    Ok(html)
}
