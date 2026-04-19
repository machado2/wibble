use axum::extract::Query;
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use sea_orm::{EntityTrait, PaginatorTrait, QueryOrder, QuerySelect};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::entities::{content as content_entity, prelude::*};
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
    Router::new().route("/admin/articles", get(get_admin_articles))
}

#[derive(Deserialize)]
struct AdminArticleQuery {
    sort: Option<String>,
    page: Option<u64>,
}

async fn get_admin_articles(
    wr: WibbleRequest,
    Query(query): Query<AdminArticleQuery>,
) -> Result<Html<String>, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    if !auth_user.is_admin() {
        return Err(Error::Auth("Admin access required".to_string()));
    }

    let db = &wr.state.db;
    let page = query.page.unwrap_or(1).max(1);
    let per_page: u64 = 50;
    let offset = (page - 1) * per_page;

    let sort_column = match query.sort.as_deref() {
        Some("title") => content_entity::Column::Title,
        Some("author") => content_entity::Column::AuthorEmail,
        Some("clicks") => content_entity::Column::ClickCount,
        Some("impressions") => content_entity::Column::ImpressionCount,
        Some("hot") => content_entity::Column::HotScore,
        Some("votes") => content_entity::Column::Votes,
        Some("generating") => content_entity::Column::Generating,
        Some("published") => content_entity::Column::Published,
        Some("fail_count") => content_entity::Column::FailCount,
        _ => content_entity::Column::CreatedAt,
    };

    let articles = Content::find()
        .order_by_desc(sort_column)
        .offset(offset)
        .limit(per_page)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading articles: {}", e)))?;

    let total = Content::find()
        .count(db)
        .await
        .map_err(|e| Error::Database(format!("Error counting articles: {}", e)))?;
    let total_pages = (total as u64).div_ceil(per_page);

    let articles_data: Vec<_> = articles
        .iter()
        .map(|article| {
            serde_json::json!({
                "id": article.id,
                "slug": article.slug,
                "title": article.title,
                "description": article.description,
                "author_email": article.author_email,
                "user_input": article.user_input,
                "model": article.model,
                "created_at": article.created_at.format("%F %T").to_string(),
                "generating": article.generating,
                "published": article.published,
                "recovered_from_dead_link": article.recovered_from_dead_link,
                "flagged": article.flagged,
                "click_count": article.click_count,
                "impression_count": article.impression_count,
                "votes": article.votes,
                "hot_score": format!("{:.2}", article.hot_score),
                "fail_count": article.fail_count,
                "generation_time_ms": article.generation_time_ms,
                "image_prompt": article.image_prompt,
            })
        })
        .collect();

    let current_sort = query.sort.as_deref().unwrap_or("created_at");
    wr.template("admin_articles")
        .await
        .insert("title", "Admin - Articles")
        .insert("robots", "noindex,nofollow")
        .insert("articles", &articles_data)
        .insert("current_sort", current_sort)
        .insert("current_page", &page)
        .insert("total_pages", &total_pages)
        .insert("has_prev", &(page > 1))
        .insert("has_next", &(page < total_pages))
        .render()
}
