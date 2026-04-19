use std::env;

use axum::extract::{Path, Query};
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Form, Router};
use chrono::TimeDelta;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, TransactionTrait,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::content as content_page;
use crate::entities::{content as content_entity, content_comment, content_vote, prelude::*};
use crate::error::Error;
use crate::hot_score::calculate_hot_score;
use crate::services::article_language::{
    requested_article_language_query_value, resolve_article_language,
    resolve_requested_article_language,
};
use crate::wibble_request::WibbleRequest;

const ARTICLE_LANGUAGE_COOKIE_NAME: &str = "__article_lang";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/content/{slug}", get(get_content))
        .route("/content/{slug}/vote", post(post_vote))
        .route("/content/{slug}/comments", post(post_comment))
}

#[derive(Deserialize)]
struct ContentQuery {
    source: Option<String>,
    comments_page: Option<u64>,
    lang: Option<String>,
}

#[derive(Deserialize)]
struct PostCommentData {
    body: String,
    lang: Option<String>,
}

#[derive(Deserialize)]
struct VoteData {
    direction: String,
    lang: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VoteAction {
    Up,
    Down,
    Clear,
}

async fn get_content(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Query(query): Query<ContentQuery>,
) -> Result<Response, Error> {
    let requested_language = resolve_requested_article_language(query.lang.as_deref());
    if let Some(raw_language) = query.lang.as_deref() {
        let canonical_language = requested_language.map(requested_article_language_query_value);
        if canonical_language != Some(raw_language) {
            return Ok(Redirect::to(&content_location_with_query(
                &slug,
                query.source.as_deref(),
                query.comments_page,
                canonical_language,
                None,
            ))
            .into_response());
        }
    }
    let requested_language = requested_language.unwrap_or(None);
    let automatic_selection =
        resolve_article_language(None, None, wr.browser_translation_language, &[]);
    let cookie_header = article_language_cookie_header(
        &slug,
        requested_language,
        automatic_selection.preferred_language.code,
        query.lang.is_some(),
    );
    let mut content_request = wr.clone();
    if query.lang.is_some() && requested_language.is_none() {
        content_request.saved_article_language = None;
    }
    let response = content_page::GetContent::get_content(
        &content_request,
        &slug,
        query.source.as_deref(),
        query.comments_page,
        requested_language,
    )
    .await?;

    Ok(match cookie_header {
        Some(cookie) => ([(SET_COOKIE, cookie)], response).into_response(),
        None => response.into_response(),
    })
}

fn content_location(slug: &str, lang: Option<&str>, anchor: Option<&str>) -> String {
    content_location_with_query(slug, None, None, lang, anchor)
}

fn content_location_with_query(
    slug: &str,
    source: Option<&str>,
    comments_page: Option<u64>,
    lang: Option<&str>,
    anchor: Option<&str>,
) -> String {
    let mut path = format!("/content/{}", slug);
    let mut query = Vec::new();
    if let Some(source) = source {
        query.push(format!("source={}", source));
    }
    if let Some(comments_page) = comments_page {
        query.push(format!("comments_page={}", comments_page));
    }
    if let Some(lang) = lang {
        query.push(format!("lang={}", lang));
    }
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    if let Some(anchor) = anchor {
        path.push('#');
        path.push_str(anchor);
    }
    path
}

fn article_language_cookie_path(slug: &str) -> String {
    format!("/content/{}", slug)
}

fn article_language_cookie(slug: &str, value: &str, max_age: u64) -> String {
    let secure = if env::var("SITE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string())
        .starts_with("https://")
    {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={}; Path={}; SameSite=Lax{}; Max-Age={}",
        ARTICLE_LANGUAGE_COOKIE_NAME,
        value,
        article_language_cookie_path(slug),
        secure,
        max_age
    )
}

fn clear_article_language_cookie(slug: &str) -> String {
    article_language_cookie(slug, "", 0)
}

fn article_language_cookie_header(
    slug: &str,
    requested_language: Option<crate::llm::prompt_registry::SupportedTranslationLanguage>,
    automatic_language_code: &str,
    update_requested: bool,
) -> Option<String> {
    if !update_requested {
        return None;
    }

    match requested_language {
        None => Some(clear_article_language_cookie(slug)),
        Some(language) => {
            if language.code == automatic_language_code {
                Some(clear_article_language_cookie(slug))
            } else {
                Some(article_language_cookie(
                    slug,
                    language.code,
                    30 * 24 * 60 * 60,
                ))
            }
        }
    }
}

fn parse_vote_action(raw: &str) -> Result<VoteAction, Error> {
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

async fn post_comment(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<PostCommentData>,
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
        &slug,
        resolve_requested_article_language(data.lang.as_deref())
            .flatten()
            .map(|language| language.code),
        Some("comments"),
    )))
}

async fn post_vote(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<VoteData>,
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
        &slug,
        resolve_requested_article_language(data.lang.as_deref())
            .flatten()
            .map(|language| language.code),
        Some("article-voting"),
    )))
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;

    use super::{article_language_cookie_header, content_location, content_location_with_query};

    #[test]
    fn content_location_preserves_language_query() {
        assert_eq!(
            content_location("test-story", Some("pt"), None),
            "/content/test-story?lang=pt"
        );
    }

    #[test]
    fn content_location_appends_anchor_after_language_query() {
        assert_eq!(
            content_location("test-story", Some("pt"), Some("comments")),
            "/content/test-story?lang=pt#comments"
        );
    }

    #[test]
    fn content_location_with_query_preserves_existing_params() {
        assert_eq!(
            content_location_with_query(
                "test-story",
                Some("top"),
                Some(3),
                Some("pt"),
                Some("comments"),
            ),
            "/content/test-story?source=top&comments_page=3&lang=pt#comments"
        );
    }

    #[test]
    fn article_language_cookie_header_sets_manual_article_cookie() {
        let cookie = article_language_cookie_header(
            "test-story",
            find_supported_translation_language("pt"),
            "en",
            true,
        )
        .unwrap();

        assert!(cookie.contains("__article_lang=pt"));
        assert!(cookie.contains("Path=/content/test-story"));
    }

    #[test]
    fn article_language_cookie_header_clears_cookie_for_automatic_mode() {
        let cookie = article_language_cookie_header("test-story", None, "en", true).unwrap();

        assert!(cookie.contains("__article_lang="));
        assert!(cookie.contains("Max-Age=0"));
    }

    #[test]
    fn article_language_cookie_header_skips_cookie_when_no_query_was_provided() {
        assert!(article_language_cookie_header("test-story", None, "en", false).is_none());
    }

    #[test]
    fn article_language_cookie_header_clears_cookie_when_choice_matches_automatic_language() {
        let cookie = article_language_cookie_header(
            "test-story",
            find_supported_translation_language("pt"),
            "pt",
            true,
        )
        .unwrap();

        assert!(cookie.contains("Max-Age=0"));
    }
}
