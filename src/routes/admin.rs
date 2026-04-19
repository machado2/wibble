use std::collections::{HashMap, HashSet};

use axum::extract::{Path, Query};
use axum::response::{Html, Redirect};
use axum::routing::{get, post};
use axum::Router;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::entities::{
    article_job, audit_log, content as content_entity, prelude::*, translation_job,
};
use crate::error::Error;
use crate::services::article_jobs::{
    ArticleJobService, ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED,
    ARTICLE_JOB_STATUS_FAILED, ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
};
use crate::translation_jobs::{
    cancel_translation_job, TRANSLATION_JOB_STATUS_CANCELLED, TRANSLATION_JOB_STATUS_COMPLETED,
    TRANSLATION_JOB_STATUS_FAILED, TRANSLATION_JOB_STATUS_PROCESSING,
    TRANSLATION_JOB_STATUS_QUEUED,
};
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
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

#[derive(Serialize)]
struct ContentMeta {
    slug: String,
    title: String,
}

#[derive(Serialize)]
struct StatusCount {
    label: &'static str,
    count: u64,
}

#[derive(Serialize)]
struct RequesterSummary {
    requester_key: String,
    requester_tier: String,
    jobs: u64,
    failures: u64,
    active: u64,
    last_seen: String,
}

#[derive(Serialize)]
struct FeatureUsageSummary {
    feature_type: String,
    jobs: u64,
    model_calls: u64,
    tool_calls: u64,
    searches: u64,
    sources: u64,
    fetched_content_chars: u64,
}

#[derive(Serialize)]
struct AuditActionSummary {
    action: String,
    count: u64,
    latest_at: String,
}

#[derive(Serialize)]
struct ArticleJobRow {
    id: String,
    article_id: Option<String>,
    title: Option<String>,
    slug: Option<String>,
    prompt: String,
    requester_key: String,
    requester_tier: String,
    feature_type: String,
    phase: String,
    status: String,
    fail_count: i32,
    updated_at: String,
    error_summary: Option<String>,
    publication_state: Option<String>,
    image_total: Option<u64>,
    image_completed: Option<u64>,
    image_failed: Option<u64>,
    can_cancel: bool,
}

#[derive(Serialize)]
struct TranslationJobRow {
    id: String,
    article_id: String,
    article_title: Option<String>,
    article_slug: Option<String>,
    language_code: String,
    request_source: String,
    priority: i32,
    status: String,
    fail_count: i32,
    updated_at: String,
    next_retry_at: Option<String>,
    last_error: Option<String>,
    can_cancel: bool,
}

async fn get_admin_articles(
    wr: WibbleRequest,
    Query(query): Query<AdminArticleQuery>,
) -> Result<Html<String>, Error> {
    require_admin_user(&wr)?;

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
                "created_at": format_time(article.created_at),
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

async fn get_admin_jobs(wr: WibbleRequest) -> Result<Html<String>, Error> {
    require_admin_user(&wr)?;
    let db = &wr.state.db;

    let article_jobs_recent = load_recent_article_jobs(db, 200).await?;
    let translation_jobs_recent = load_recent_translation_jobs(db, 200).await?;
    let active_article_jobs = load_article_jobs_by_statuses(
        db,
        &[ARTICLE_JOB_STATUS_QUEUED, ARTICLE_JOB_STATUS_PROCESSING],
        50,
    )
    .await?;
    let failed_article_jobs = load_article_jobs_by_statuses(
        db,
        &[ARTICLE_JOB_STATUS_FAILED, ARTICLE_JOB_STATUS_CANCELLED],
        50,
    )
    .await?;
    let active_translation_jobs = load_translation_jobs_by_statuses(
        db,
        &[
            TRANSLATION_JOB_STATUS_QUEUED,
            TRANSLATION_JOB_STATUS_PROCESSING,
        ],
        50,
    )
    .await?;
    let failed_translation_jobs = load_translation_jobs_by_statuses(
        db,
        &[
            TRANSLATION_JOB_STATUS_FAILED,
            TRANSLATION_JOB_STATUS_CANCELLED,
        ],
        50,
    )
    .await?;

    let content_map = load_content_metadata_map(
        db,
        article_jobs_recent
            .iter()
            .filter_map(article_job_content_id)
            .chain(
                translation_jobs_recent
                    .iter()
                    .map(|job| job.article_id.clone()),
            ),
    )
    .await?;

    let article_status_counts = vec![
        StatusCount {
            label: "queued",
            count: count_article_jobs_by_status(db, ARTICLE_JOB_STATUS_QUEUED).await?,
        },
        StatusCount {
            label: "processing",
            count: count_article_jobs_by_status(db, ARTICLE_JOB_STATUS_PROCESSING).await?,
        },
        StatusCount {
            label: "completed",
            count: count_article_jobs_by_status(db, ARTICLE_JOB_STATUS_COMPLETED).await?,
        },
        StatusCount {
            label: "failed",
            count: count_article_jobs_by_status(db, ARTICLE_JOB_STATUS_FAILED).await?,
        },
        StatusCount {
            label: "cancelled",
            count: count_article_jobs_by_status(db, ARTICLE_JOB_STATUS_CANCELLED).await?,
        },
    ];
    let translation_status_counts = vec![
        StatusCount {
            label: "queued",
            count: count_translation_jobs_by_status(db, TRANSLATION_JOB_STATUS_QUEUED).await?,
        },
        StatusCount {
            label: "processing",
            count: count_translation_jobs_by_status(db, TRANSLATION_JOB_STATUS_PROCESSING).await?,
        },
        StatusCount {
            label: "completed",
            count: count_translation_jobs_by_status(db, TRANSLATION_JOB_STATUS_COMPLETED).await?,
        },
        StatusCount {
            label: "failed",
            count: count_translation_jobs_by_status(db, TRANSLATION_JOB_STATUS_FAILED).await?,
        },
        StatusCount {
            label: "cancelled",
            count: count_translation_jobs_by_status(db, TRANSLATION_JOB_STATUS_CANCELLED).await?,
        },
    ];

    let requester_summaries = build_requester_summaries(&article_jobs_recent);
    let feature_usage = build_feature_usage_summaries(&article_jobs_recent);
    let audit_summaries = load_recent_audit_action_summaries(db, 200).await?;
    let rate_limit_metrics = wr.state.rate_limit_state.admin_snapshot();

    let active_article_rows = active_article_jobs
        .iter()
        .map(|job| article_job_row(job, &content_map))
        .collect::<Vec<_>>();
    let failed_article_rows = failed_article_jobs
        .iter()
        .map(|job| article_job_row(job, &content_map))
        .collect::<Vec<_>>();
    let active_translation_rows = active_translation_jobs
        .iter()
        .map(|job| translation_job_row(job, &content_map))
        .collect::<Vec<_>>();
    let failed_translation_rows = failed_translation_jobs
        .iter()
        .map(|job| translation_job_row(job, &content_map))
        .collect::<Vec<_>>();

    wr.template("admin_jobs")
        .await
        .insert("title", "Admin - Job Monitor")
        .insert("robots", "noindex,nofollow")
        .insert("article_status_counts", &article_status_counts)
        .insert("translation_status_counts", &translation_status_counts)
        .insert("requester_summaries", &requester_summaries)
        .insert("feature_usage", &feature_usage)
        .insert("audit_summaries", &audit_summaries)
        .insert("rate_limit_metrics", &rate_limit_metrics)
        .insert("active_article_jobs", &active_article_rows)
        .insert("failed_article_jobs", &failed_article_rows)
        .insert("active_translation_jobs", &active_translation_rows)
        .insert("failed_translation_jobs", &failed_translation_rows)
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
    Ok(Redirect::to("/admin/jobs"))
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
    Ok(Redirect::to("/admin/jobs"))
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

async fn load_recent_article_jobs(
    db: &DatabaseConnection,
    limit: u64,
) -> Result<Vec<article_job::Model>, Error> {
    ArticleJob::find()
        .order_by_desc(article_job::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article jobs: {}", e)))
}

async fn load_recent_translation_jobs(
    db: &DatabaseConnection,
    limit: u64,
) -> Result<Vec<translation_job::Model>, Error> {
    TranslationJob::find()
        .order_by_desc(translation_job::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading translation jobs: {}", e)))
}

async fn load_article_jobs_by_statuses(
    db: &DatabaseConnection,
    statuses: &[&str],
    limit: u64,
) -> Result<Vec<article_job::Model>, Error> {
    ArticleJob::find()
        .filter(article_job::Column::Status.is_in(statuses.iter().copied()))
        .order_by_desc(article_job::Column::UpdatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading filtered article jobs: {}", e)))
}

async fn load_translation_jobs_by_statuses(
    db: &DatabaseConnection,
    statuses: &[&str],
    limit: u64,
) -> Result<Vec<translation_job::Model>, Error> {
    TranslationJob::find()
        .filter(translation_job::Column::Status.is_in(statuses.iter().copied()))
        .order_by_desc(translation_job::Column::UpdatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading filtered translation jobs: {}", e)))
}

async fn count_article_jobs_by_status(db: &DatabaseConnection, status: &str) -> Result<u64, Error> {
    ArticleJob::find()
        .filter(article_job::Column::Status.eq(status))
        .count(db)
        .await
        .map_err(|e| Error::Database(format!("Error counting article jobs: {}", e)))
}

async fn count_translation_jobs_by_status(
    db: &DatabaseConnection,
    status: &str,
) -> Result<u64, Error> {
    TranslationJob::find()
        .filter(translation_job::Column::Status.eq(status))
        .count(db)
        .await
        .map_err(|e| Error::Database(format!("Error counting translation jobs: {}", e)))
}

async fn load_content_metadata_map<I>(
    db: &DatabaseConnection,
    ids: I,
) -> Result<HashMap<String, ContentMeta>, Error>
where
    I: IntoIterator<Item = String>,
{
    let ids = ids.into_iter().collect::<HashSet<_>>();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let content = Content::find()
        .filter(content_entity::Column::Id.is_in(ids.iter().cloned()))
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading content metadata: {}", e)))?;

    Ok(content
        .into_iter()
        .map(|article| {
            (
                article.id,
                ContentMeta {
                    slug: article.slug,
                    title: article.title,
                },
            )
        })
        .collect())
}

fn article_job_content_id(job: &article_job::Model) -> Option<String> {
    job.article_id
        .clone()
        .or_else(|| parse_string_field(job.preview_payload.as_deref(), "article_id"))
        .or_else(|| {
            if job.status == ARTICLE_JOB_STATUS_COMPLETED
                || job.status == ARTICLE_JOB_STATUS_PROCESSING
            {
                Some(job.id.clone())
            } else {
                None
            }
        })
}

fn article_job_row(
    job: &article_job::Model,
    content_map: &HashMap<String, ContentMeta>,
) -> ArticleJobRow {
    let content_id = article_job_content_id(job);
    let content_meta = content_id.as_ref().and_then(|id| content_map.get(id));

    ArticleJobRow {
        id: job.id.clone(),
        article_id: content_id,
        title: parse_string_field(job.preview_payload.as_deref(), "title")
            .or_else(|| content_meta.map(|meta| meta.title.clone())),
        slug: parse_string_field(job.preview_payload.as_deref(), "slug")
            .or_else(|| content_meta.map(|meta| meta.slug.clone())),
        prompt: job.prompt.clone(),
        requester_key: job.requester_key.clone(),
        requester_tier: job.requester_tier.clone(),
        feature_type: job.feature_type.clone(),
        phase: job.phase.clone(),
        status: job.status.clone(),
        fail_count: job.fail_count,
        updated_at: format_time(job.updated_at),
        error_summary: job.error_summary.clone(),
        publication_state: parse_string_field(job.preview_payload.as_deref(), "publication_state"),
        image_total: parse_u64_field(job.preview_payload.as_deref(), "image_total"),
        image_completed: parse_u64_field(job.preview_payload.as_deref(), "image_completed"),
        image_failed: parse_u64_field(job.preview_payload.as_deref(), "image_failed"),
        can_cancel: matches!(
            job.status.as_str(),
            ARTICLE_JOB_STATUS_QUEUED | ARTICLE_JOB_STATUS_PROCESSING
        ),
    }
}

fn translation_job_row(
    job: &translation_job::Model,
    content_map: &HashMap<String, ContentMeta>,
) -> TranslationJobRow {
    let content_meta = content_map.get(&job.article_id);
    TranslationJobRow {
        id: job.id.clone(),
        article_id: job.article_id.clone(),
        article_title: content_meta.map(|meta| meta.title.clone()),
        article_slug: content_meta.map(|meta| meta.slug.clone()),
        language_code: job.language_code.clone(),
        request_source: job.request_source.clone(),
        priority: job.priority,
        status: job.status.clone(),
        fail_count: job.fail_count,
        updated_at: format_time(job.updated_at),
        next_retry_at: job.next_retry_at.map(format_time),
        last_error: job.last_error.clone(),
        can_cancel: matches!(
            job.status.as_str(),
            TRANSLATION_JOB_STATUS_QUEUED
                | TRANSLATION_JOB_STATUS_PROCESSING
                | TRANSLATION_JOB_STATUS_FAILED
        ),
    }
}

fn build_requester_summaries(jobs: &[article_job::Model]) -> Vec<RequesterSummary> {
    let mut by_requester = HashMap::<String, RequesterSummary>::new();
    for job in jobs {
        let entry = by_requester
            .entry(job.requester_key.clone())
            .or_insert_with(|| RequesterSummary {
                requester_key: job.requester_key.clone(),
                requester_tier: job.requester_tier.clone(),
                jobs: 0,
                failures: 0,
                active: 0,
                last_seen: format_time(job.updated_at),
            });
        entry.jobs += 1;
        if matches!(
            job.status.as_str(),
            ARTICLE_JOB_STATUS_FAILED | ARTICLE_JOB_STATUS_CANCELLED
        ) {
            entry.failures += 1;
        }
        if matches!(
            job.status.as_str(),
            ARTICLE_JOB_STATUS_QUEUED | ARTICLE_JOB_STATUS_PROCESSING
        ) {
            entry.active += 1;
        }
        let updated_at = format_time(job.updated_at);
        if updated_at > entry.last_seen {
            entry.last_seen = updated_at;
        }
    }

    let mut rows = by_requester.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .jobs
            .cmp(&left.jobs)
            .then_with(|| right.failures.cmp(&left.failures))
            .then_with(|| left.requester_key.cmp(&right.requester_key))
    });
    rows.truncate(10);
    rows
}

fn build_feature_usage_summaries(jobs: &[article_job::Model]) -> Vec<FeatureUsageSummary> {
    let mut by_feature = HashMap::<String, FeatureUsageSummary>::new();
    for job in jobs {
        let entry = by_feature
            .entry(job.feature_type.clone())
            .or_insert_with(|| FeatureUsageSummary {
                feature_type: job.feature_type.clone(),
                jobs: 0,
                model_calls: 0,
                tool_calls: 0,
                searches: 0,
                sources: 0,
                fetched_content_chars: 0,
            });
        entry.jobs += 1;
        entry.model_calls += usage_counter(job, "model_calls");
        entry.tool_calls += usage_counter(job, "tool_calls");
        entry.searches += usage_counter(job, "searches");
        entry.sources += usage_counter(job, "sources");
        entry.fetched_content_chars += usage_counter(job, "fetched_content_chars");
    }

    let mut rows = by_feature.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| left.feature_type.cmp(&right.feature_type));
    rows
}

async fn load_recent_audit_action_summaries(
    db: &DatabaseConnection,
    limit: u64,
) -> Result<Vec<AuditActionSummary>, Error> {
    let logs = AuditLog::find()
        .order_by_desc(audit_log::Column::CreatedAt)
        .limit(limit)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading audit logs: {}", e)))?;

    let mut grouped = HashMap::<String, AuditActionSummary>::new();
    for log in logs {
        let entry = grouped
            .entry(log.action.clone())
            .or_insert_with(|| AuditActionSummary {
                action: log.action.clone(),
                count: 0,
                latest_at: format_time(log.created_at),
            });
        entry.count += 1;
    }

    let mut rows = grouped.into_values().collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.action.cmp(&right.action))
    });
    rows.truncate(12);
    Ok(rows)
}

fn usage_counter(job: &article_job::Model, key: &str) -> u64 {
    job.usage_counters
        .as_deref()
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.get(key).and_then(Value::as_u64))
        .unwrap_or(0)
}

fn parse_string_field(value: Option<&str>, key: &str) -> Option<String> {
    value
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.get(key).and_then(Value::as_str).map(str::to_string))
}

fn parse_u64_field(value: Option<&str>, key: &str) -> Option<u64> {
    value
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.get(key).and_then(Value::as_u64))
}

fn format_time(value: chrono::NaiveDateTime) -> String {
    value.format("%F %T").to_string()
}
