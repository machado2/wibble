mod service;

use axum::extract::{Path, Query};
use axum::response::{Html, Redirect};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::error::Error;
use crate::services::article_jobs::ArticleJobService;
use crate::translation_jobs::cancel_translation_job;
use crate::wibble_request::WibbleRequest;

use self::service::{load_admin_articles_page, load_admin_jobs_page};

pub fn localized_router() -> Router<AppState> {
    Router::new()
        .route("/admin/articles", get(get_admin_articles))
        .route("/admin/jobs", get(get_admin_jobs))
        .route(
            "/admin/article-jobs/{id}/cancel",
            post(post_cancel_article_job),
        )
        .route(
            "/admin/translation-jobs/{id}/cancel",
            post(post_cancel_translation_job),
        )
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
    require_admin_user(&wr)?;
    let page = load_admin_articles_page(&wr.state.db, query.sort.as_deref(), query.page).await?;

    wr.template("admin_articles")
        .await
        .insert("title", "Admin - Articles")
        .insert("robots", "noindex,nofollow")
        .insert("articles", &page.articles)
        .insert("current_sort", &page.current_sort)
        .insert("current_page", &page.current_page)
        .insert("total_pages", &page.total_pages)
        .insert("has_prev", &page.has_prev)
        .insert("has_next", &page.has_next)
        .render()
}

async fn get_admin_jobs(wr: WibbleRequest) -> Result<Html<String>, Error> {
    require_admin_user(&wr)?;
    let page = load_admin_jobs_page(&wr.state).await?;

    wr.template("admin_jobs")
        .await
        .insert("title", "Admin - Job Monitor")
        .insert("robots", "noindex,nofollow")
        .insert("article_status_counts", &page.article_status_counts)
        .insert("translation_status_counts", &page.translation_status_counts)
        .insert("requester_summaries", &page.requester_summaries)
        .insert("feature_usage", &page.feature_usage)
        .insert("audit_summaries", &page.audit_summaries)
        .insert("rate_limit_metrics", &page.rate_limit_metrics)
        .insert("active_article_jobs", &page.active_article_jobs)
        .insert("failed_article_jobs", &page.failed_article_jobs)
        .insert("active_translation_jobs", &page.active_translation_jobs)
        .insert("failed_translation_jobs", &page.failed_translation_jobs)
        .render()
}

async fn post_cancel_article_job(
    wr: WibbleRequest,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    let auth_user = require_admin_user(&wr)?;
    let job = ArticleJobService::new(wr.state.clone())
        .cancel_job(&id, "Cancelled by admin")
        .await?;
    if job.is_none() {
        return Err(Error::NotFound(Some(format!(
            "Article job {} not found",
            id
        ))));
    }
    let details = serde_json::json!({
        "reason": "cancelled_by_admin",
    })
    .to_string();
    log_audit(
        &wr.state.db,
        auth_user,
        "cancel_article_job",
        "article_job",
        &id,
        Some(details),
    )
    .await?;
    Ok(Redirect::to(&wr.localized_path("/admin/jobs")))
}

async fn post_cancel_translation_job(
    wr: WibbleRequest,
    Path(id): Path<String>,
) -> Result<Redirect, Error> {
    let auth_user = require_admin_user(&wr)?;
    if !cancel_translation_job(&wr.state, &id).await? {
        return Err(Error::NotFound(Some(format!(
            "Translation job {} not found",
            id
        ))));
    }
    let details = serde_json::json!({
        "reason": "cancelled_by_admin",
    })
    .to_string();
    log_audit(
        &wr.state.db,
        auth_user,
        "cancel_translation_job",
        "translation_job",
        &id,
        Some(details),
    )
    .await?;
    Ok(Redirect::to(&wr.localized_path("/admin/jobs")))
}

fn require_admin_user(wr: &WibbleRequest) -> Result<&AuthUser, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    if !auth_user.is_admin() {
        return Err(Error::Auth("Admin access required".to_string()));
    }
    Ok(auth_user)
}
