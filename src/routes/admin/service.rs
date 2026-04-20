use std::collections::{HashMap, HashSet};

use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::Serialize;
use serde_json::Value;

use crate::app_state::AppState;
use crate::entities::{
    article_job, audit_log, content as content_entity, prelude::*, translation_job,
};
use crate::error::Error;
use crate::rate_limit::RateLimitMetricsSnapshot;
use crate::services::article_jobs::{
    ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED, ARTICLE_JOB_STATUS_FAILED,
    ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
};
use crate::translation_jobs::{
    TRANSLATION_JOB_STATUS_CANCELLED, TRANSLATION_JOB_STATUS_COMPLETED,
    TRANSLATION_JOB_STATUS_FAILED, TRANSLATION_JOB_STATUS_PROCESSING,
    TRANSLATION_JOB_STATUS_QUEUED,
};

#[derive(Serialize)]
pub(super) struct AdminArticleRow {
    id: String,
    slug: String,
    title: String,
    description: String,
    author_email: Option<String>,
    user_input: String,
    model: String,
    created_at: String,
    generating: bool,
    published: bool,
    recovered_from_dead_link: bool,
    flagged: bool,
    click_count: i32,
    impression_count: i32,
    votes: i32,
    hot_score: String,
    fail_count: i32,
    generation_time_ms: Option<i32>,
    image_prompt: Option<String>,
}

#[derive(Serialize)]
pub(super) struct ContentMeta {
    slug: String,
    title: String,
}

#[derive(Serialize)]
pub(super) struct StatusCount {
    label: &'static str,
    count: u64,
}

#[derive(Serialize)]
pub(super) struct RequesterSummary {
    requester_key: String,
    requester_tier: String,
    jobs: u64,
    failures: u64,
    active: u64,
    last_seen: String,
}

#[derive(Serialize)]
pub(super) struct FeatureUsageSummary {
    feature_type: String,
    jobs: u64,
    model_calls: u64,
    tool_calls: u64,
    searches: u64,
    sources: u64,
    fetched_content_chars: u64,
}

#[derive(Serialize)]
pub(super) struct AuditActionSummary {
    action: String,
    count: u64,
    latest_at: String,
}

#[derive(Serialize)]
pub(super) struct ArticleJobRow {
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
pub(super) struct TranslationJobRow {
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

pub(super) struct AdminArticlesPageData {
    pub(super) articles: Vec<AdminArticleRow>,
    pub(super) current_sort: String,
    pub(super) current_page: u64,
    pub(super) total_pages: u64,
    pub(super) has_prev: bool,
    pub(super) has_next: bool,
}

pub(super) struct AdminJobsPageData {
    pub(super) article_status_counts: Vec<StatusCount>,
    pub(super) translation_status_counts: Vec<StatusCount>,
    pub(super) requester_summaries: Vec<RequesterSummary>,
    pub(super) feature_usage: Vec<FeatureUsageSummary>,
    pub(super) audit_summaries: Vec<AuditActionSummary>,
    pub(super) rate_limit_metrics: RateLimitMetricsSnapshot,
    pub(super) active_article_jobs: Vec<ArticleJobRow>,
    pub(super) failed_article_jobs: Vec<ArticleJobRow>,
    pub(super) active_translation_jobs: Vec<TranslationJobRow>,
    pub(super) failed_translation_jobs: Vec<TranslationJobRow>,
}

pub(super) async fn load_admin_articles_page(
    db: &DatabaseConnection,
    sort: Option<&str>,
    page: Option<u64>,
) -> Result<AdminArticlesPageData, Error> {
    let page = page.unwrap_or(1).max(1);
    let per_page: u64 = 50;
    let offset = (page - 1) * per_page;

    let sort_column = match sort {
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

    Ok(AdminArticlesPageData {
        articles: articles.into_iter().map(admin_article_row).collect(),
        current_sort: sort.unwrap_or("created_at").to_string(),
        current_page: page,
        total_pages,
        has_prev: page > 1,
        has_next: page < total_pages,
    })
}

pub(super) async fn load_admin_jobs_page(state: &AppState) -> Result<AdminJobsPageData, Error> {
    let db = &state.db;
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

    Ok(AdminJobsPageData {
        article_status_counts: load_article_status_counts(db).await?,
        translation_status_counts: load_translation_status_counts(db).await?,
        requester_summaries: build_requester_summaries(&article_jobs_recent),
        feature_usage: build_feature_usage_summaries(&article_jobs_recent),
        audit_summaries: load_recent_audit_action_summaries(db, 200).await?,
        rate_limit_metrics: state.rate_limit_state.admin_snapshot(),
        active_article_jobs: active_article_jobs
            .iter()
            .map(|job| article_job_row(job, &content_map))
            .collect(),
        failed_article_jobs: failed_article_jobs
            .iter()
            .map(|job| article_job_row(job, &content_map))
            .collect(),
        active_translation_jobs: active_translation_jobs
            .iter()
            .map(|job| translation_job_row(job, &content_map))
            .collect(),
        failed_translation_jobs: failed_translation_jobs
            .iter()
            .map(|job| translation_job_row(job, &content_map))
            .collect(),
    })
}

fn admin_article_row(article: content_entity::Model) -> AdminArticleRow {
    AdminArticleRow {
        id: article.id,
        slug: article.slug,
        title: article.title,
        description: article.description,
        author_email: article.author_email,
        user_input: article.user_input,
        model: article.model,
        created_at: format_time(article.created_at),
        generating: article.generating,
        published: article.published,
        recovered_from_dead_link: article.recovered_from_dead_link,
        flagged: article.flagged,
        click_count: article.click_count,
        impression_count: article.impression_count,
        votes: article.votes,
        hot_score: format!("{:.2}", article.hot_score),
        fail_count: article.fail_count,
        generation_time_ms: article.generation_time_ms,
        image_prompt: article.image_prompt,
    }
}

async fn load_article_status_counts(db: &DatabaseConnection) -> Result<Vec<StatusCount>, Error> {
    Ok(vec![
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
    ])
}

async fn load_translation_status_counts(
    db: &DatabaseConnection,
) -> Result<Vec<StatusCount>, Error> {
    Ok(vec![
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
    ])
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
    let mut by_requester = HashMap::<String, (RequesterSummary, chrono::NaiveDateTime)>::new();
    for job in jobs {
        let entry = by_requester
            .entry(job.requester_key.clone())
            .or_insert_with(|| {
                (
                    RequesterSummary {
                        requester_key: job.requester_key.clone(),
                        requester_tier: job.requester_tier.clone(),
                        jobs: 0,
                        failures: 0,
                        active: 0,
                        last_seen: format_time(job.updated_at),
                    },
                    job.updated_at,
                )
            });
        entry.0.jobs += 1;
        if matches!(
            job.status.as_str(),
            ARTICLE_JOB_STATUS_FAILED | ARTICLE_JOB_STATUS_CANCELLED
        ) {
            entry.0.failures += 1;
        }
        if matches!(
            job.status.as_str(),
            ARTICLE_JOB_STATUS_QUEUED | ARTICLE_JOB_STATUS_PROCESSING
        ) {
            entry.0.active += 1;
        }
        if job.updated_at > entry.1 {
            entry.1 = job.updated_at;
            entry.0.last_seen = format_time(job.updated_at);
        }
    }

    let mut rows = by_requester
        .into_values()
        .map(|(summary, _)| summary)
        .collect::<Vec<_>>();
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

    let mut grouped = HashMap::<String, (AuditActionSummary, chrono::NaiveDateTime)>::new();
    for log in logs {
        let entry = grouped.entry(log.action.clone()).or_insert_with(|| {
            (
                AuditActionSummary {
                    action: log.action.clone(),
                    count: 0,
                    latest_at: format_time(log.created_at),
                },
                log.created_at,
            )
        });
        entry.0.count += 1;
        if log.created_at > entry.1 {
            entry.1 = log.created_at;
            entry.0.latest_at = format_time(log.created_at);
        }
    }

    let mut rows = grouped
        .into_values()
        .map(|(summary, _)| summary)
        .collect::<Vec<_>>();
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
