#![allow(non_snake_case)]
#![allow(clippy::blocks_in_conditions)]

use axum::response::Html;
use chrono::TimeDelta;
use sea_orm::ColumnTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use sea_orm::{prelude::*, FromQueryResult, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};

use crate::entities::{content, prelude::*};
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

#[derive(Default, Deserialize, Debug, Clone)]
pub struct ContentListParams {
    pub afterId: Option<String>,
    pub pageSize: Option<u8>,
    pub search: Option<String>,
    pub t: Option<String>,
    pub sort: Option<String>,
}

#[derive(DerivePartialModel, FromQueryResult, Serialize)]
#[sea_orm(entity = "Content")]
pub struct Headline {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    #[sea_orm(unique)]
    pub slug: String,
    pub created_at: DateTime,
    pub description: String,
    pub image_id: Option<String>,
    pub title: String,
}

async fn get_next_page(
    db: &DatabaseConnection,
    par: ContentListParams,
) -> Result<Vec<Headline>, Error> {
    let r: Result<_, DbErr> = async {
        let page_size = match par.pageSize {
            Some(i) if i < 100 => i,
            _ => 20,
        };

        let mut contents = Content::find()
            .filter(content::Column::Flagged.eq(false))
            .filter(content::Column::Generating.eq(false));
        contents = match par.search {
            Some(s) => contents.filter(
                content::Column::Slug
                    .contains(&s)
                    .or(content::Column::Title.contains(&s))
                    .or(content::Column::Description.contains(&s))
                    .or(content::Column::Content.contains(&s)),
            ),
            None => contents,
        };
        let days = match par.t.unwrap_or_default().as_str() {
            "week" => TimeDelta::try_weeks(1),
            "month" => TimeDelta::try_days(30),
            _ => None,
        };
        if let Some(days) = days {
            contents = contents
                .filter(content::Column::CreatedAt.gt(chrono::Utc::now().naive_utc() - days));
        }
        let sort_column = match par.sort {
            Some(s) if s == "most_viewed" => content::Column::ViewCount,
            Some(s) if s == "hot" => content::Column::HotScore,
            _ => content::Column::CreatedAt,
        };

        let after_content = match par.afterId {
            Some(id) => {
                Content::find()
                    .filter(content::Column::Id.eq(id))
                    .one(db)
                    .await?
            }
            None => None,
        };
        let contents = match after_content {
            Some(ac) => contents
                .filter(
                    content::Column::Id
                        .ne(ac.id.clone())
                        .and(sort_column.lte(ac.get(sort_column))),
                )
                .filter(
                    sort_column
                        .lt(ac.get(sort_column))
                        .or(content::Column::Id.lt(ac.id.clone())),
                ),
            None => contents,
        };

        let contents = contents
            .order_by_desc(sort_column)
            .order_by_desc(content::Column::Id)
            .limit(page_size as u64)
            .into_partial_model::<Headline>()
            .all(db)
            .await?;
        Ok(contents)
    }
    .await;
    r.map_err(|e| Error::Database(format!("Error getting next page: {}", e)))
}

#[derive(Serialize)]
struct FormattedHeadline {
    id: String,
    slug: String,
    created_at: String,
    description: String,
    image_id: Option<String>,
    title: String,
}

fn format_headline(h: Headline) -> FormattedHeadline {
    FormattedHeadline {
        id: h.id,
        slug: h.slug,
        created_at: h.created_at.format("%F").to_string(),
        description: h.description,
        image_id: h.image_id,
        title: h.title,
    }
}

#[allow(async_fn_in_trait)]
pub trait NewsList {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error>;
}

impl NewsList for WibbleRequest {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error> {
        let db = &self.state.db;
        let items = get_next_page(db, params).await?;
        let items: Vec<_> = items.into_iter().map(format_headline).collect();
        let after_id = items.last().map(|h| h.id.clone());
        self.template("index")
            .await
            .insert("items", &items)
            .insert("after_id", &after_id)
            .render()
    }
}
