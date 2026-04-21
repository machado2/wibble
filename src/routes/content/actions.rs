use axum::extract::Path;
use axum::response::Redirect;
use axum::Form;
use chrono::TimeDelta;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, TransactionTrait,
};
use uuid::Uuid;

use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::content as content_page;
use crate::entities::{content as content_entity, content_comment, content_vote, prelude::*};
use crate::error::Error;
use crate::hot_score::calculate_hot_score;
use crate::services::article_language::resolve_requested_article_language;
use crate::wibble_request::WibbleRequest;

use super::navigation::content_location;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum VoteAction {
    Up,
    Down,
    Clear,
}

pub(super) fn parse_vote_action(raw: &str) -> Result<VoteAction, Error> {
    match raw {
        "up" => Ok(VoteAction::Up),
        "down" => Ok(VoteAction::Down),
        "clear" => Ok(VoteAction::Clear),
        _ => Err(Error::BadRequest("Invalid vote direction".to_string())),
    }
}

fn comment_min_interval_seconds() -> i64 {
    std::env::var("COMMENT_MIN_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(30)
        .max(0)
}

fn comment_max_per_hour() -> u64 {
    std::env::var("COMMENT_MAX_PER_HOUR")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20)
}

async fn enforce_comment_rate_limit(
    db: &DatabaseConnection,
    auth_user: &AuthUser,
) -> Result<(), Error> {
    let now = chrono::Utc::now().naive_local();
    let min_interval_seconds = comment_min_interval_seconds();
    if min_interval_seconds > 0 {
        let recent_cutoff = now - TimeDelta::seconds(min_interval_seconds);
        let recent_comments = ContentComment::find()
            .filter(content_comment::Column::UserEmail.eq(auth_user.email.clone()))
            .filter(content_comment::Column::CreatedAt.gte(recent_cutoff))
            .count(db)
            .await
            .map_err(|e| Error::Database(format!("Error checking recent comments: {}", e)))?;
        if recent_comments > 0 {
            return Err(Error::RateLimited);
        }
    }

    let max_per_hour = comment_max_per_hour();
    if max_per_hour > 0 {
        let hourly_cutoff = now - TimeDelta::hours(1);
        let hourly_comments = ContentComment::find()
            .filter(content_comment::Column::UserEmail.eq(auth_user.email.clone()))
            .filter(content_comment::Column::CreatedAt.gte(hourly_cutoff))
            .count(db)
            .await
            .map_err(|e| Error::Database(format!("Error checking comment rate limit: {}", e)))?;
        if hourly_comments >= max_per_hour {
            return Err(Error::RateLimited);
        }
    }

    Ok(())
}

pub(super) async fn post_comment(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<super::PostCommentData>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let body = content_page::normalize_comment_body(&data.body)?;
    let db = &wr.state.db;
    let article = content_page::require_article_by_slug(db, &slug).await?;
    if !content_page::article_accepts_public_interactions(&article) {
        return Err(Error::BadRequest(
            "Comments are only available on published articles".to_string(),
        ));
    }
    enforce_comment_rate_limit(db, auth_user).await?;

    let comment = content_comment::Model {
        id: Uuid::new_v4().to_string(),
        content_id: article.id,
        user_email: auth_user.email.clone(),
        user_name: auth_user.name.clone(),
        body,
        created_at: chrono::Utc::now().naive_local(),
    };

    ContentComment::insert(content_comment::ActiveModel::from(comment))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting comment: {}", e)))?;

    log_audit(db, auth_user, "create_comment", "content", &slug, None).await?;

    Ok(Redirect::to(&content_location(
        wr.site_language,
        &slug,
        resolve_requested_article_language(data.lang.as_deref())
            .flatten()
            .map(|language| language.code),
        Some("comments"),
    )))
}

pub(super) async fn post_vote(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<super::VoteData>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let action = parse_vote_action(&data.direction)?;
    let db = &wr.state.db;
    let article = content_page::require_article_by_slug(db, &slug).await?;
    if !content_page::article_accepts_public_interactions(&article) {
        return Err(Error::BadRequest(
            "Voting is only available on published articles".to_string(),
        ));
    }

    let vote_user = auth_user.clone();
    let audit_user = auth_user.clone();
    let article_id = article.id.clone();
    let created_at = article.created_at;
    match db
        .transaction::<_, (), Error>(|tx| {
            Box::pin(async move {
                let existing =
                    ContentVote::find_by_id((article_id.clone(), vote_user.email.clone()))
                        .one(tx)
                        .await
                        .map_err(|e| Error::Database(format!("Error loading vote: {}", e)))?;

                match (action, existing) {
                    (VoteAction::Up, Some(existing)) if !existing.downvote => {
                        ContentVote::delete_by_id((article_id.clone(), vote_user.email.clone()))
                            .exec(tx)
                            .await
                            .map_err(|e| Error::Database(format!("Error clearing vote: {}", e)))?;
                    }
                    (VoteAction::Down, Some(existing)) if existing.downvote => {
                        ContentVote::delete_by_id((article_id.clone(), vote_user.email.clone()))
                            .exec(tx)
                            .await
                            .map_err(|e| Error::Database(format!("Error clearing vote: {}", e)))?;
                    }
                    (VoteAction::Clear, Some(_)) => {
                        ContentVote::delete_by_id((article_id.clone(), vote_user.email.clone()))
                            .exec(tx)
                            .await
                            .map_err(|e| Error::Database(format!("Error clearing vote: {}", e)))?;
                    }
                    (VoteAction::Up, Some(existing)) => {
                        let mut active: content_vote::ActiveModel = existing.into();
                        active.downvote = ActiveValue::set(false);
                        active
                            .update(tx)
                            .await
                            .map_err(|e| Error::Database(format!("Error updating vote: {}", e)))?;
                    }
                    (VoteAction::Down, Some(existing)) => {
                        let mut active: content_vote::ActiveModel = existing.into();
                        active.downvote = ActiveValue::set(true);
                        active
                            .update(tx)
                            .await
                            .map_err(|e| Error::Database(format!("Error updating vote: {}", e)))?;
                    }
                    (VoteAction::Up, None) => {
                        ContentVote::insert(content_vote::ActiveModel {
                            content_id: ActiveValue::set(article_id.clone()),
                            user_email: ActiveValue::set(vote_user.email.clone()),
                            created_at: ActiveValue::set(chrono::Utc::now().naive_local()),
                            downvote: ActiveValue::set(false),
                        })
                        .exec(tx)
                        .await
                        .map_err(|e| Error::Database(format!("Error inserting vote: {}", e)))?;
                    }
                    (VoteAction::Down, None) => {
                        ContentVote::insert(content_vote::ActiveModel {
                            content_id: ActiveValue::set(article_id.clone()),
                            user_email: ActiveValue::set(vote_user.email.clone()),
                            created_at: ActiveValue::set(chrono::Utc::now().naive_local()),
                            downvote: ActiveValue::set(true),
                        })
                        .exec(tx)
                        .await
                        .map_err(|e| Error::Database(format!("Error inserting vote: {}", e)))?;
                    }
                    (VoteAction::Clear, None) => {}
                }

                let upvotes = ContentVote::find()
                    .filter(content_vote::Column::ContentId.eq(article_id.clone()))
                    .filter(content_vote::Column::Downvote.eq(false))
                    .count(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error counting upvotes: {}", e)))?;
                let downvotes = ContentVote::find()
                    .filter(content_vote::Column::ContentId.eq(article_id.clone()))
                    .filter(content_vote::Column::Downvote.eq(true))
                    .count(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error counting downvotes: {}", e)))?;
                let vote_score = (upvotes as i64 - downvotes as i64)
                    .clamp(i32::MIN as i64, i32::MAX as i64)
                    as i32;
                let hot_score =
                    calculate_hot_score(vote_score, created_at, chrono::Utc::now().naive_local());

                Content::update_many()
                    .filter(content_entity::Column::Id.eq(article_id))
                    .col_expr(content_entity::Column::Votes, Expr::value(vote_score))
                    .col_expr(content_entity::Column::HotScore, Expr::value(hot_score))
                    .exec(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error updating article score: {}", e)))?;

                Ok(())
            })
        })
        .await
    {
        Ok(()) => {}
        Err(sea_orm::TransactionError::Connection(e)) => {
            return Err(Error::Database(format!(
                "Error applying vote transaction: {}",
                e
            )))
        }
        Err(sea_orm::TransactionError::Transaction(e)) => return Err(e),
    }

    log_audit(
        db,
        &audit_user,
        "vote_article",
        "content",
        &slug,
        Some(data.direction),
    )
    .await?;

    Ok(Redirect::to(&content_location(
        wr.site_language,
        &slug,
        resolve_requested_article_language(data.lang.as_deref())
            .flatten()
            .map(|language| language.code),
        Some("article-voting"),
    )))
}
