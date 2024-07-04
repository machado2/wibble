use axum::extract::Path;
use axum::response::Html;
use sea_orm::EntityTrait;
use serde_json::Value;

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

    let mut template = wr.template("image_info").await;
    let parameters = info.parameters.clone();
    let mut parameters_inserted = false;
    if let Some(pars) = parameters {
        let pars = serde_json::from_str(&pars);
        if let Ok(pars) = pars {
            let pars = serde_json::to_string_pretty::<Value>(&pars);
            if let Ok(pars) = pars {
                template.insert("parameters", &pars);
                parameters_inserted = true;
            }
        }
    }
    if !parameters_inserted {
        template.insert("parameters", &info.parameters);
    }

    template
        .insert("image_id", &id)
        .insert("title", &info.alt_text)
        .insert("created_at", &info.created_at.format("%F").to_string())
        .insert("slug", &slug)
        .insert("content_title", &article_title)
        .insert("prompt", &info.prompt)
        .insert("model", &info.model)
        .render()
}
