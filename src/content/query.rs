use axum::response::Html;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use tracing::{event, warn, Level};

use crate::auth::AuthUser;
use crate::create::{render_wait_page, start_recover_article_for_slug};
use crate::entities::{content, content_image, prelude::*};
use crate::error::Error;
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;

use super::policy::can_view_article;

pub enum ContentPageArticle {
    Ready(Box<content::Model>),
    Wait(Html<String>),
}

fn content_not_found(slug: &str) -> Error {
    Error::NotFound(Some(format!("Content with slug {} not found", slug)))
}

fn article_not_found(slug: &str) -> Error {
    Error::NotFound(Some(format!("Article {} not found", slug)))
}

async fn find_content_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<Option<content::Model>, Error> {
    Content::find()
        .filter(content::Column::Slug.eq(slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading content: {}", e)))
}

async fn clear_stale_generating_flag(
    db: &DatabaseConnection,
    article_id: &str,
) -> Result<(), Error> {
    Content::update_many()
        .filter(content::Column::Id.eq(article_id.to_string()))
        .col_expr(content::Column::Generating, Expr::value(false))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Failed to clear stale generating flag: {}", e)))?;
    Ok(())
}

async fn delete_stale_article(db: &DatabaseConnection, article_id: &str) -> Result<(), Error> {
    ContentImage::delete_many()
        .filter(content_image::Column::ContentId.eq(article_id.to_string()))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Failed to delete stale content images: {}", e)))?;
    Content::delete_by_id(article_id.to_string())
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Failed to delete stale content row: {}", e)))?;
    Ok(())
}

pub async fn find_article_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<Option<content::Model>, Error> {
    Content::find()
        .filter(content::Column::Slug.eq(slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))
}

pub async fn require_article_by_slug(
    db: &DatabaseConnection,
    slug: &str,
) -> Result<content::Model, Error> {
    find_article_by_slug(db, slug)
        .await?
        .ok_or_else(|| article_not_found(slug))
}

pub async fn find_article_after_id(
    db: &DatabaseConnection,
    slug: &str,
    after_id: Option<String>,
) -> Result<content::Model, Error> {
    Content::find()
        .filter(content::Column::Slug.contains(slug))
        .filter(content::Column::Id.gt(after_id.unwrap_or_default()))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading content: {}", e)))?
        .ok_or_else(|| content_not_found(slug))
}

pub async fn increment_click_count(db: &DatabaseConnection, article_id: &str) -> Result<(), Error> {
    Content::update_many()
        .filter(content::Column::Id.eq(article_id.to_string()))
        .col_expr(
            content::Column::ClickCount,
            Expr::col(content::Column::ClickCount).add(1),
        )
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating click count: {}", e)))?;
    Ok(())
}

pub async fn load_user_vote(
    db: &DatabaseConnection,
    article_id: &str,
    auth_user: Option<&AuthUser>,
    interactions_open: bool,
) -> Result<String, Error> {
    if !interactions_open {
        return Ok(String::new());
    }

    let Some(auth_user) = auth_user else {
        return Ok(String::new());
    };

    Ok(
        ContentVote::find_by_id((article_id.to_string(), auth_user.email.clone()))
            .one(db)
            .await
            .map_err(|e| Error::Database(format!("Error loading vote: {}", e)))?
            .map(|vote| {
                if vote.downvote {
                    "down".to_string()
                } else {
                    "up".to_string()
                }
            })
            .unwrap_or_default(),
    )
}

pub async fn load_content_page_article(
    request: &WibbleRequest,
    slug: &str,
) -> Result<ContentPageArticle, Error> {
    let state = &request.state;
    let db = &state.db;
    let mut article = find_content_by_slug(db, slug).await?;

    if article.is_none() {
        if let Err(e) = start_recover_article_for_slug(state.clone(), slug.to_string()).await {
            warn!(slug = %slug, error = %e, "Failed to start dead-link recovery");
        }
        article = find_content_by_slug(db, slug).await?;
    }

    let mut article = article.ok_or_else(|| content_not_found(slug))?;
    if !can_view_article(request.auth_user.as_ref(), &article) {
        return Err(content_not_found(slug));
    }

    if !article.generating {
        return Ok(ContentPageArticle::Ready(Box::new(article)));
    }

    let task_processing = matches!(
        state.task_list.get(&article.id).await,
        Ok(TaskResult::Processing)
    );
    if state.is_generation_active(&article.id).await || task_processing {
        event!(
            Level::INFO,
            slug = %slug,
            article_id = %article.id,
            "Serving wait page for active generation"
        );
        return Ok(ContentPageArticle::Wait(
            render_wait_page(request, &article.id).await?,
        ));
    }

    if article.markdown.is_some() {
        warn!(
            slug = %slug,
            article_id = %article.id,
            "Found stale generating row with markdown; flipping generating=false"
        );
        clear_stale_generating_flag(db, &article.id).await?;
        article.generating = false;
    } else {
        warn!(
            slug = %slug,
            article_id = %article.id,
            "Found stale generating row with no in-memory active task; removing and retrying recovery"
        );
        delete_stale_article(db, &article.id).await?;

        if let Err(e) = start_recover_article_for_slug(state.clone(), slug.to_string()).await {
            warn!(slug = %slug, error = %e, "Failed to restart dead-link recovery");
        }
        article = find_content_by_slug(db, slug)
            .await?
            .ok_or_else(|| content_not_found(slug))?;

        if !can_view_article(request.auth_user.as_ref(), &article) {
            return Err(content_not_found(slug));
        }
    }

    if article.generating {
        return Ok(ContentPageArticle::Wait(
            render_wait_page(request, &article.id).await?,
        ));
    }

    Ok(ContentPageArticle::Ready(Box::new(article)))
}
