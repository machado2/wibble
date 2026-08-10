#![allow(non_snake_case)]

use axum::response::Html;
use sea_orm::{prelude::*, FromQueryResult, QueryOrder, QuerySelect};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use url::form_urlencoded::Serializer;

use crate::entities::{content, prelude::*};
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ContentListParams {
    pub afterId: Option<String>,
    pub pageSize: Option<u8>,
}

#[derive(DerivePartialModel, FromQueryResult, Serialize)]
#[sea_orm(entity = "Content")]
struct Headline {
    #[sea_orm(primary_key, auto_increment = false)]
    id: String,
    #[sea_orm(unique)]
    slug: String,
    created_at: DateTime,
    description: String,
    image_id: Option<String>,
    title: String,
}

#[derive(Clone, Serialize)]
struct FormattedHeadline {
    id: String,
    slug: String,
    created_at: String,
    description: String,
    image_id: Option<String>,
    title: String,
}

async fn get_next_page(
    db: &DatabaseConnection,
    params: ContentListParams,
) -> Result<(Vec<FormattedHeadline>, Option<String>), Error> {
    let page_size = params.pageSize.filter(|size| *size < 100).unwrap_or(20);
    let mut query = Content::find()
        .filter(content::Column::Flagged.eq(false))
        .filter(content::Column::Generating.eq(false))
        .filter(content::Column::Published.eq(true));

    if let Some(after_id) = params.afterId {
        if let Some(after) = Content::find_by_id(after_id)
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Error loading page cursor: {}", e)))?
        {
            query = query.filter(
                content::Column::CreatedAt
                    .lt(after.created_at)
                    .or(content::Column::CreatedAt
                        .eq(after.created_at)
                        .and(content::Column::Id.lt(after.id))),
            );
        }
    }

    let mut rows = query
        .order_by_desc(content::Column::CreatedAt)
        .order_by_desc(content::Column::Id)
        .limit(page_size as u64 + 1)
        .into_partial_model::<Headline>()
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error getting next page: {}", e)))?;
    let has_more = rows.len() > page_size as usize;
    if has_more {
        rows.truncate(page_size as usize);
    }
    let next_after_id = has_more
        .then(|| rows.last().map(|row| row.id.clone()))
        .flatten();
    let rows = rows
        .into_iter()
        .map(|row| FormattedHeadline {
            id: row.id,
            slug: row.slug,
            created_at: row.created_at.format("%F").to_string(),
            description: row.description,
            image_id: row.image_id,
            title: row.title,
        })
        .collect();
    Ok((rows, next_after_id))
}

#[allow(async_fn_in_trait)]
pub trait NewsList {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error>;
}

impl NewsList for WibbleRequest {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error> {
        let (items, next_after_id) = get_next_page(&self.state.db, params).await?;
        let lead_item = items.first().cloned();
        let secondary_items = items.iter().skip(1).cloned().collect::<Vec<_>>();
        let load_more_url = next_after_id.as_deref().map_or_else(
            || "/".to_string(),
            |id| {
                let query = Serializer::new(String::new())
                    .append_pair("afterId", id)
                    .finish();
                format!("/?{}", query)
            },
        );
        let mut template = self.template("index").await;
        template
            .insert("title", "Wibble News")
            .insert("description", "The latest news from Wibble News")
            .insert("secondary_items", &secondary_items)
            .insert("has_more", &next_after_id.is_some())
            .insert("load_more_url", &load_more_url);
        if let Some(lead_item) = lead_item {
            template.insert("lead_item", &lead_item);
        }
        template.render()
    }
}
