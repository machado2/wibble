use std::env;
use std::net::Ipv4Addr;

use axum::body::{Body, Bytes};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header::SET_COOKIE;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{middleware, serve, Form, Router};
use chrono::TimeDelta;
use dotenvy::dotenv;
use rand::Rng;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, QuerySelect, TransactionTrait,
};
use serde::Deserialize;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use url::form_urlencoded::Serializer;
use uuid::Uuid;

use wibble::app_state::AppState;
use wibble::auth::AuthUser;
use wibble::content::{article_accepts_public_interactions, normalize_comment_body, GetContent};
use wibble::create::{
    normalize_create_prompt, render_create_page, start_create_article, wait, PostCreateData,
    WaitResponse,
};
use wibble::entities::{
    audit_log, content, content_comment, content_image, content_vote, prelude::*,
};
use wibble::error::Error;
use wibble::hot_score::calculate_hot_score;
use wibble::image_info::get_image_info_handler;
use wibble::newslist::{ContentListParams, NewsList};
use wibble::rate_limit::rate_limit_middleware;
use wibble::repository::store_image_file;
use wibble::wibble_request::WibbleRequest;

// #[debug_handler(state = AppState)]
async fn get_index(
    wr: WibbleRequest,
    Query(data): Query<ContentListParams>,
) -> Result<Html<String>, Error> {
    wr.news_list(data).await
}

#[derive(Deserialize)]
struct ContentQuery {
    source: Option<String>,
    comments_page: Option<u64>,
}

async fn get_content(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Query(query): Query<ContentQuery>,
) -> Result<Html<String>, Error> {
    wr.get_content(&slug, query.source.as_deref(), query.comments_page)
        .await
}

async fn get_image(wr: WibbleRequest, Path(id): Path<String>) -> Result<Response, StatusCode> {
    let img = wibble::image::get_image(&wr.state, &id, wr.auth_user.as_ref())
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    Response::builder()
        .header("Content-Type", img.content_type)
        .header("Cache-Control", img.cache_control)
        .body(Body::from(Bytes::from(img.bytes)))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_wait(wr: WibbleRequest, Path(id): Path<String>) -> Response {
    match wait(wr, &id).await {
        WaitResponse::Redirect(slug) => {
            let url = format!("/content/{}", slug);
            Redirect::to(&url).into_response()
        }
        WaitResponse::Html(html) => html.into_response(),
        WaitResponse::NotFound => StatusCode::NOT_FOUND.into_response(),
        WaitResponse::InternalError => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn site_url_from_env() -> String {
    env::var("SITE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn auth_cookie(token: &str, max_age: u64) -> String {
    let secure = if site_url_from_env().starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    format!(
        "__auth={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={}",
        token, secure, max_age
    )
}

fn sanitize_redirect_target(raw: Option<String>) -> String {
    let site_url = site_url_from_env();
    raw.and_then(|target| {
        if target.starts_with('/') && !target.starts_with("//") {
            Some(target)
        } else if target.starts_with(&site_url) {
            let relative = target[site_url.len()..].to_string();
            if relative.starts_with('/') {
                Some(relative)
            } else {
                Some("/".to_string())
            }
        } else {
            None
        }
    })
    .unwrap_or_else(|| "/".to_string())
}

async fn create_en(wr: WibbleRequest, Form(data): Form<PostCreateData>) -> impl IntoResponse {
    let author_email = wr.auth_user.as_ref().map(|u| u.email.clone());
    let prompt = match normalize_create_prompt(&data.prompt) {
        Ok(prompt) => prompt,
        Err(Error::BadRequest(message)) => {
            return match render_create_page(&wr, data.prompt.trim(), Some(&message)).await {
                Ok(html) => (StatusCode::BAD_REQUEST, html).into_response(),
                Err(e) => e.into_response(),
            };
        }
        Err(e) => return e.into_response(),
    };
    match start_create_article(wr.state, prompt, author_email).await {
        Ok(id) => Redirect::to(&format!("/wait/{}", id)).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    wibble::create::get_create(wr).await
}

#[derive(Deserialize)]
struct AuthCallbackParams {
    token: Option<String>,
    redirect: Option<String>,
}

async fn auth_callback(
    State(state): State<AppState>,
    Query(params): Query<AuthCallbackParams>,
) -> Result<Response, Error> {
    let token = params
        .token
        .ok_or_else(|| Error::Auth("Missing token".to_string()))?;
    let _user = state.jwks_client.validate_token(&token).await?;
    let cookie = auth_cookie(&token, 30 * 24 * 60 * 60);
    let redirect_url = sanitize_redirect_target(params.redirect);
    Ok(([(SET_COOKIE, cookie)], Redirect::to(&redirect_url)).into_response())
}

async fn login() -> Redirect {
    let auth_url =
        env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "https://auth.fbmac.net".to_string());
    let callback_url = format!(
        "{}/auth/callback",
        site_url_from_env().trim_end_matches('/')
    );
    let query = Serializer::new(String::new())
        .append_pair("redirect", &callback_url)
        .append_pair("mode", "both")
        .finish();
    Redirect::to(&format!("{}/login?{}", auth_url, query))
}

async fn logout() -> Response {
    let auth_url =
        env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "https://auth.fbmac.net".to_string());
    let our_url = site_url_from_env();
    let cookie = auth_cookie("", 0);
    let query = Serializer::new(String::new())
        .append_pair("redirect", &our_url)
        .finish();
    (
        [(SET_COOKIE, cookie)],
        Redirect::to(&format!("{}/logout?{}", auth_url, query)),
    )
        .into_response()
}

#[derive(Deserialize)]
struct PostCommentData {
    body: String,
}

#[derive(Deserialize)]
struct VoteData {
    direction: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VoteAction {
    Up,
    Down,
    Clear,
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
    env::var("COMMENT_MIN_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(30)
        .max(0)
}

fn comment_max_per_hour() -> u64 {
    env::var("COMMENT_MAX_PER_HOUR")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20)
}

async fn enforce_comment_rate_limit(
    db: &sea_orm::DatabaseConnection,
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

fn can_edit_article(auth_user: &AuthUser, _article: &content::Model) -> bool {
    auth_user.is_admin()
}

fn can_toggle_publish(auth_user: &AuthUser, article: &content::Model) -> bool {
    if auth_user.is_admin() {
        return true;
    }
    article.author_email.as_deref() == Some(&auth_user.email)
}

async fn log_audit(
    db: &sea_orm::DatabaseConnection,
    user: &AuthUser,
    action: &str,
    target_type: &str,
    target_id: &str,
    details: Option<String>,
) -> Result<(), Error> {
    let id = Uuid::new_v4().to_string();
    let now = chrono::Utc::now().naive_local();
    let log = audit_log::Model {
        id,
        user_email: user.email.clone(),
        user_name: Some(user.name.clone()),
        action: action.to_string(),
        target_type: target_type.to_string(),
        target_id: target_id.to_string(),
        details,
        created_at: now,
    };
    AuditLog::insert(audit_log::ActiveModel::from(log))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting audit log: {}", e)))?;
    Ok(())
}

async fn get_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Html<String>, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(&article.id))
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading images: {}", e)))?;

    let image_data: Vec<_> = images
        .iter()
        .map(|img| {
            serde_json::json!({
                "id": img.id,
                "alt_text": img.alt_text,
                "prompt": img.prompt,
            })
        })
        .collect();

    wr.template("edit")
        .await
        .insert("title", &format!("Edit: {}", article.title))
        .insert("robots", "noindex,nofollow")
        .insert("article_title", &article.title)
        .insert("article_description", &article.description)
        .insert(
            "article_markdown",
            article.markdown.as_deref().unwrap_or(""),
        )
        .insert("slug", &slug)
        .insert("id", &article.id)
        .insert("images", &image_data)
        .render()
}

#[derive(Deserialize, Debug)]
struct EditArticleData {
    title: String,
    description: String,
    markdown: String,
}

async fn post_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<EditArticleData>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    let mut active: content::ActiveModel = article.into();
    active.title = ActiveValue::set(data.title.clone());
    active.description = ActiveValue::set(data.description);
    active.markdown = ActiveValue::set(Some(data.markdown.clone()));
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating article: {}", e)))?;

    log_audit(db, auth_user, "edit_article", "content", &slug, None).await?;

    Ok(Redirect::to(&format!("/content/{}", slug)))
}

async fn post_replace_image(
    wr: WibbleRequest,
    Path((slug, image_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;

    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth("Not authorized".to_string()));
    }

    let img = ContentImage::find_by_id(image_id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding image: {}", e)))?
        .ok_or(Error::NotFound(Some(format!(
            "Image {} not found",
            image_id
        ))))?;

    if img.content_id != article.id {
        return Err(Error::Auth(
            "Image does not belong to this article".to_string(),
        ));
    }

    let mut image_data = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        if name == "image" {
            let data = field
                .bytes()
                .await
                .map_err(|e| Error::Auth(format!("Failed to read upload: {}", e)))?;
            image_data = Some(data.to_vec());
        }
    }
    let image_data = image_data.ok_or_else(|| Error::Auth("No image uploaded".to_string()))?;

    store_image_file(&image_id, image_data).await?;

    log_audit(
        db,
        auth_user,
        "replace_image",
        "content_image",
        &image_id,
        Some(format!("article={}", slug)),
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}/edit", slug)))
}

async fn post_toggle_publish(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_toggle_publish(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to toggle publish state".to_string(),
        ));
    }

    let new_state = !article.published;
    let mut active: content::ActiveModel = article.into();
    active.published = ActiveValue::set(new_state);
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating publish state: {}", e)))?;

    log_audit(
        db,
        auth_user,
        if new_state {
            "publish_article"
        } else {
            "unpublish_article"
        },
        "content",
        &slug,
        None,
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}", slug)))
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
    let body = normalize_comment_body(&data.body)?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;
    if !article_accepts_public_interactions(&article) {
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

    Ok(Redirect::to(&format!("/content/{}#comments", slug)))
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
    let article = Content::find()
        .filter(content::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;
    if !article_accepts_public_interactions(&article) {
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
                            ..Default::default()
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
                            ..Default::default()
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
                    .filter(content::Column::Id.eq(article_id))
                    .col_expr(content::Column::Votes, Expr::value(vote_score))
                    .col_expr(content::Column::HotScore, Expr::value(hot_score))
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

    Ok(Redirect::to(&format!("/content/{}#article-voting", slug)))
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
        Some("title") => content::Column::Title,
        Some("author") => content::Column::AuthorEmail,
        Some("clicks") => content::Column::ClickCount,
        Some("impressions") => content::Column::ImpressionCount,
        Some("hot") => content::Column::HotScore,
        Some("votes") => content::Column::Votes,
        Some("generating") => content::Column::Generating,
        Some("published") => content::Column::Published,
        Some("fail_count") => content::Column::FailCount,
        _ => content::Column::CreatedAt,
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
        .map(|a| {
            serde_json::json!({
                "id": a.id,
                "slug": a.slug,
                "title": a.title,
                "description": a.description,
                "author_email": a.author_email,
                "user_input": a.user_input,
                "model": a.model,
                "created_at": a.created_at.format("%F %T").to_string(),
                "generating": a.generating,
                "published": a.published,
                "recovered_from_dead_link": a.recovered_from_dead_link,
                "flagged": a.flagged,
                "click_count": a.click_count,
                "impression_count": a.impression_count,
                "votes": a.votes,
                "hot_score": format!("{:.2}", a.hot_score),
                "fail_count": a.fail_count,
                "generation_time_ms": a.generation_time_ms,
                "image_prompt": a.image_prompt,
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

async fn handle_error(
    wr: WibbleRequest,
    req: axum::http::Request<Body>,
    next: Next,
) -> impl IntoResponse {
    let response = next.run(req).await;
    let status_code = response.status();
    match status_code {
        StatusCode::INTERNAL_SERVER_ERROR => {
            let image_url = format!("/error{}.jpg", rand::rng().random_range(1..=8));
            wr.template("error")
                .await
                .insert("title", "Server error")
                .insert(
                    "description",
                    "An unexpected server error occurred while loading this page.",
                )
                .insert("robots", "noindex,nofollow")
                .insert("image_url", &image_url)
                .insert(
                    "error_message",
                    "Oops! Something went wrong. Please try again later.",
                )
                .render()
                .into_response()
        }
        StatusCode::NOT_FOUND => {
            let image_url = format!("/notfound{}.jpg", rand::rng().random_range(1..=4));
            wr.template("error")
                .await
                .insert("title", "Page not found")
                .insert("description", "The requested page could not be found.")
                .insert("robots", "noindex,nofollow")
                .insert("image_url", &image_url)
                .insert(
                    "error_message",
                    "The page you are looking for does not exist.",
                )
                .render()
                .into_response()
        }
        _ => response,
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let port: u16 = env::var("PORT")
        .unwrap_or("8000".to_string())
        .parse()
        .unwrap();
    let serve_dir = ServeDir::new("static");
    let state = AppState::init()
        .await
        .unwrap_or_else(|e| panic!("Failed to initialize application state: {}", e));
    let app = Router::new()
        .route("/", get(get_index))
        .route("/sitemap.xml", get(wibble::sitemap::get_sitemap))
        .route("/robots.txt", get(wibble::sitemap::get_robots_txt))
        .route("/image/{id}", get(get_image))
        .route("/image_info/{id}", get(get_image_info_handler))
        .route("/content/{slug}", get(get_content))
        .route("/content/{slug}/vote", post(post_vote))
        .route("/content/{slug}/comments", post(post_comment))
        .route(
            "/content/{slug}/edit",
            get(get_edit_article).post(post_edit_article),
        )
        .route(
            "/content/{slug}/images/{image_id}",
            post(post_replace_image),
        )
        .route("/wait/{id}", get(get_wait))
        .route("/create", post(create_en).get(get_create))
        .route("/images", get(wibble::get_images::get_images))
        .route("/admin/articles", get(get_admin_articles))
        .route("/content/{slug}/publish", post(post_toggle_publish))
        .route("/auth/callback", get(auth_callback))
        .route("/login", get(login))
        .route("/logout", get(logout))
        .fallback_service(serve_dir)
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn_with_state(
            state.rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .layer(middleware::from_fn_with_state(state.clone(), handle_error))
        .with_state(state);
    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, port))
        .await
        .unwrap();
    serve(listener, app.into_make_service()).await.unwrap();
}
