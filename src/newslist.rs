#![allow(non_snake_case)]
#![allow(clippy::blocks_in_conditions)]

use axum::response::Html;
use chrono::TimeDelta;
use sea_orm::sea_query::Expr;
use sea_orm::ColumnTrait;
use sea_orm::EntityTrait;
use sea_orm::QueryFilter;
use sea_orm::{prelude::*, FromQueryResult, QueryOrder, QuerySelect};
use serde::{Deserialize, Serialize};
use url::form_urlencoded::Serializer;

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

fn public_sort_column(sort: Option<&str>) -> content::Column {
    match sort {
        Some("hot") => content::Column::HotScore,
        _ => content::Column::CreatedAt,
    }
}

async fn get_next_page(
    db: &DatabaseConnection,
    par: ContentListParams,
) -> Result<(Vec<Headline>, Option<String>), Error> {
    let r: Result<_, DbErr> = async {
        let page_size = match par.pageSize {
            Some(i) if i < 100 => i,
            _ => 20,
        };

        let mut contents = Content::find()
            .filter(content::Column::Flagged.eq(false))
            .filter(content::Column::Generating.eq(false))
            .filter(content::Column::Published.eq(true));
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
        // Public ranking ignores click data because it is too noisy.
        let sort_column = public_sort_column(par.sort.as_deref());

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
            .limit(page_size as u64 + 1)
            .into_partial_model::<Headline>()
            .all(db)
            .await?;
        let mut contents = contents;
        let has_more = contents.len() > page_size as usize;
        if has_more {
            contents.truncate(page_size as usize);
        }
        let next_after_id = if has_more {
            contents.last().map(|headline| headline.id.clone())
        } else {
            None
        };
        Ok((contents, next_after_id))
    }
    .await;
    r.map_err(|e| Error::Database(format!("Error getting next page: {}", e)))
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

#[derive(Serialize)]
struct FilterOption {
    label: &'static str,
    url: String,
    active: bool,
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

fn build_index_url(
    search: Option<&str>,
    t: Option<&str>,
    sort: Option<&str>,
    after_id: Option<&str>,
) -> String {
    let mut serializer = Serializer::new(String::new());
    if let Some(search) = search.filter(|value| !value.trim().is_empty()) {
        serializer.append_pair("search", search);
    }
    if let Some(t) = t.filter(|value| !value.is_empty()) {
        serializer.append_pair("t", t);
    }
    if let Some(sort) = sort.filter(|value| !value.is_empty()) {
        serializer.append_pair("sort", sort);
    }
    if let Some(after_id) = after_id.filter(|value| !value.is_empty()) {
        serializer.append_pair("afterId", after_id);
    }
    let query = serializer.finish();
    if query.is_empty() {
        "/".to_string()
    } else {
        format!("/?{}", query)
    }
}

fn sort_options(params: &ContentListParams) -> [FilterOption; 2] {
    let search = params.search.as_deref();
    let t = params.t.as_deref();
    let current_sort = params.sort.as_deref().unwrap_or("new");
    [
        FilterOption {
            label: "Newest",
            url: build_index_url(search, t, None, None),
            active: current_sort != "hot",
        },
        FilterOption {
            label: "Hot",
            url: build_index_url(search, t, Some("hot"), None),
            active: current_sort == "hot",
        },
    ]
}

fn time_options(params: &ContentListParams) -> [FilterOption; 3] {
    let search = params.search.as_deref();
    let sort = params.sort.as_deref();
    let current_time = params.t.as_deref().unwrap_or("");
    [
        FilterOption {
            label: "Any time",
            url: build_index_url(search, None, sort, None),
            active: current_time.is_empty(),
        },
        FilterOption {
            label: "This week",
            url: build_index_url(search, Some("week"), sort, None),
            active: current_time == "week",
        },
        FilterOption {
            label: "This month",
            url: build_index_url(search, Some("month"), sort, None),
            active: current_time == "month",
        },
    ]
}

#[allow(async_fn_in_trait)]
pub trait NewsList {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error>;
}

impl NewsList for WibbleRequest {
    async fn news_list(&self, params: ContentListParams) -> Result<Html<String>, Error> {
        let db = &self.state.db;
        let search = params.search.clone();
        let has_filters = params.afterId.is_some()
            || params.search.is_some()
            || params.t.is_some()
            || params.sort.is_some();
        let has_active_search = params
            .search
            .as_ref()
            .is_some_and(|search| !search.trim().is_empty());
        // Ordering is performed in SQL. Do not re-sort in Rust.
        let (items, next_after_id) = get_next_page(db, params.clone()).await?;
        let top_ids: Vec<String> = items.iter().take(3).map(|h| h.id.clone()).collect();
        if !top_ids.is_empty() {
            Content::update_many()
                .filter(content::Column::Id.is_in(top_ids))
                .col_expr(
                    content::Column::ImpressionCount,
                    Expr::col(content::Column::ImpressionCount).add(1),
                )
                .exec(db)
                .await
                .map_err(|e| Error::Database(format!("Error updating impressions: {}", e)))?;
        }
        let mut items: Vec<_> = items.into_iter().map(format_headline).collect();
        let has_results = !items.is_empty();
        let lead_item = if items.is_empty() {
            None
        } else {
            Some(items.remove(0))
        };
        let mut template = self.template("index").await;
        let title = match search {
            Some(search) if !search.trim().is_empty() => format!("Search results for {}", search),
            _ => "Latest Wibble News".to_string(),
        };
        let description =
            "Dry official bulletins on civic confusion, institutional overreaction, and preventable emergencies.";
        let load_more_url = build_index_url(
            params.search.as_deref(),
            params.t.as_deref(),
            params.sort.as_deref(),
            next_after_id.as_deref(),
        );
        let sort_options = sort_options(&params);
        let time_options = time_options(&params);
        template
            .insert("items", &items)
            .insert("load_more_url", &load_more_url)
            .insert("has_more", &next_after_id.is_some())
            .insert("title", &title)
            .insert("description", description)
            .insert("current_search", &params.search.clone().unwrap_or_default())
            .insert(
                "current_sort_key",
                &params.sort.clone().unwrap_or_else(|| "new".to_string()),
            )
            .insert("current_time_key", &params.t.clone().unwrap_or_default())
            .insert("sort_options", &sort_options)
            .insert("time_options", &time_options)
            .insert("has_results", &has_results)
            .insert("has_active_search", &has_active_search)
            .insert("secondary_items", &items)
            .insert(
                "reset_filters_url",
                &build_index_url(None, None, None, None),
            );
        if let Some(lead_item) = lead_item {
            template.insert("lead_item", &lead_item);
        }
        if has_filters {
            template.insert("robots", "noindex,follow");
        }
        template.render()
    }
}

#[cfg(test)]
mod tests {
    use super::{build_index_url, public_sort_column};
    use crate::entities::content;
    use std::mem::discriminant;

    #[test]
    fn hot_sort_uses_hot_score() {
        assert_eq!(
            discriminant(&public_sort_column(Some("hot"))),
            discriminant(&content::Column::HotScore)
        );
    }

    #[test]
    fn most_viewed_falls_back_to_newest() {
        assert_eq!(
            discriminant(&public_sort_column(Some("most_viewed"))),
            discriminant(&content::Column::CreatedAt)
        );
    }

    #[test]
    fn build_index_url_preserves_active_filters() {
        assert_eq!(
            build_index_url(Some("space mayor"), Some("week"), Some("hot"), Some("abc")),
            "/?search=space+mayor&t=week&sort=hot&afterId=abc"
        );
        assert_eq!(build_index_url(None, None, None, None), "/");
    }
}
