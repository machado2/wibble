use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder, QuerySelect};
use serde_json::Value;
use std::env;
use std::future::Future;
use std::pin::Pin;
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::article_id::normalize_content_model;
use crate::create::create_article;
use crate::entities::{article_job, content, content_image, prelude::*};
use crate::error::Error;
use crate::image_status::{is_pending_status, IMAGE_STATUS_COMPLETED, IMAGE_STATUS_FAILED};

use super::definitions::{
    ArticleJobFeatureType, ArticleJobTrace, ImageProgress, ARTICLE_JOB_STATUS_PROCESSING,
    ARTICLE_JOB_STATUS_QUEUED,
};

type BoxedArticleFuture = Pin<Box<dyn Future<Output = Result<(), Error>> + Send>>;

pub(super) async fn due_article_job_ids(state: &AppState) -> Result<Vec<String>, Error> {
    ArticleJob::find()
        .filter(
            article_job::Column::Status
                .is_in([ARTICLE_JOB_STATUS_QUEUED, ARTICLE_JOB_STATUS_PROCESSING]),
        )
        .order_by_asc(article_job::Column::CreatedAt)
        .limit(article_job_resume_batch_size())
        .select_only()
        .column(article_job::Column::Id)
        .into_tuple::<String>()
        .all(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading pending article jobs: {}", e)))
}

pub(super) async fn load_article_job(
    state: &AppState,
    job_id: &str,
) -> Result<Option<article_job::Model>, Error> {
    ArticleJob::find_by_id(job_id.to_string())
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article job {}: {}", job_id, e)))
}

pub(super) async fn load_article_job_for_article(
    state: &AppState,
    article_id: &str,
) -> Result<Option<article_job::Model>, Error> {
    ArticleJob::find()
        .filter(
            Condition::any()
                .add(article_job::Column::Id.eq(article_id.to_string()))
                .add(article_job::Column::ArticleId.eq(article_id.to_string())),
        )
        .order_by_desc(article_job::Column::CreatedAt)
        .one(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!(
                "Error loading article job for article {}: {}",
                article_id, e
            ))
        })
}

pub(super) async fn load_job_article_content(
    state: &AppState,
    job: &article_job::Model,
) -> Result<Option<content::Model>, Error> {
    let mut candidate_ids = Vec::new();
    if let Some(article_id) = job.article_id.as_deref() {
        candidate_ids.push(article_id.to_string());
    }
    if job.id != job.article_id.as_deref().unwrap_or_default() {
        candidate_ids.push(job.id.clone());
    }

    for article_id in candidate_ids {
        if let Some(article) = Content::find_by_id(article_id.clone())
            .one(&state.db)
            .await
            .map_err(|e| {
                Error::Database(format!(
                    "Error loading article content {}: {}",
                    article_id, e
                ))
            })?
        {
            return Ok(Some(normalize_content_model(article)));
        }
    }

    Ok(None)
}

pub(super) async fn load_article_image_progress(
    state: &AppState,
    article_id: &str,
) -> Result<ImageProgress, Error> {
    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(article_id.to_string()))
        .all(&state.db)
        .await
        .map_err(|e| {
            Error::Database(format!(
                "Error loading article images for article {}: {}",
                article_id, e
            ))
        })?;

    let mut progress = ImageProgress {
        total: images.len(),
        ..ImageProgress::default()
    };
    for image in images {
        if image.status == IMAGE_STATUS_COMPLETED {
            progress.completed += 1;
        } else if image.status == IMAGE_STATUS_FAILED {
            progress.failed += 1;
        } else if is_pending_status(&image.status) {
            progress.processing += 1;
            progress.pending_ids.push(image.id);
        }
    }

    Ok(progress)
}

pub(super) fn build_generation_future(
    state: AppState,
    job: &article_job::Model,
) -> Result<BoxedArticleFuture, Error> {
    let Some(feature_type) = ArticleJobFeatureType::from_str(&job.feature_type) else {
        return Err(Error::BadRequest(format!(
            "Unsupported article job feature type: {}",
            job.feature_type
        )));
    };
    let job_id = job.id.clone();
    let prompt = job.prompt.clone();
    let author_email = job.author_email.clone();
    let research_mode = feature_type.research_mode();
    Ok(Box::pin(async move {
        create_article(&state, job_id, prompt, author_email, research_mode).await
    }))
}

pub(super) fn default_usage_counters(prompt_chars: usize) -> Value {
    serde_json::json!({
        "prompt_chars": prompt_chars,
        "agent_steps": 0,
        "model_calls": 0,
        "tool_calls": 0,
        "searches": 0,
        "sources": 0,
        "fetched_content_chars": 0,
        "image_total": 0,
        "image_completed": 0,
        "image_processing": 0,
        "image_failed": 0,
    })
}

fn parse_usage_counters(existing: Option<&str>, prompt_chars: usize) -> Value {
    existing
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .filter(|value| value.is_object())
        .unwrap_or_else(|| default_usage_counters(prompt_chars))
}

pub(super) fn merge_job_usage_counters(
    existing: Option<&str>,
    prompt: &str,
    progress: &ImageProgress,
) -> String {
    let prompt_chars = prompt.chars().count();
    let mut value = parse_usage_counters(existing, prompt_chars);
    let object = value
        .as_object_mut()
        .expect("usage counter payload must stay an object");
    object.insert("prompt_chars".to_string(), Value::from(prompt_chars));
    object.insert(
        "image_total".to_string(),
        Value::from(progress.total as u64),
    );
    object.insert(
        "image_completed".to_string(),
        Value::from(progress.completed as u64),
    );
    object.insert(
        "image_processing".to_string(),
        Value::from(progress.processing as u64),
    );
    object.insert(
        "image_failed".to_string(),
        Value::from(progress.failed as u64),
    );
    value.to_string()
}

pub(super) fn build_job_preview_payload(
    existing: Option<&str>,
    article: &content::Model,
    progress: &ImageProgress,
) -> String {
    let mut payload = existing
        .and_then(|value| serde_json::from_str::<Value>(value).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default();
    let publication_state = if article.published { "public" } else { "draft" };
    payload.insert("article_id".to_string(), Value::from(article.id.clone()));
    payload.insert("slug".to_string(), Value::from(article.slug.clone()));
    payload.insert("title".to_string(), Value::from(article.title.clone()));
    payload.insert(
        "publication_state".to_string(),
        Value::from(publication_state),
    );
    payload.insert(
        "image_total".to_string(),
        Value::from(progress.total as u64),
    );
    payload.insert(
        "image_completed".to_string(),
        Value::from(progress.completed as u64),
    );
    payload.insert(
        "image_failed".to_string(),
        Value::from(progress.failed as u64),
    );
    Value::Object(payload).to_string()
}

pub(super) fn article_job_resume_interval_seconds() -> u64 {
    env::var("ARTICLE_JOB_RESUME_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(10)
}

fn article_job_resume_batch_size() -> u64 {
    env::var("ARTICLE_JOB_RESUME_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50)
}

pub(super) fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_local()
}

pub(super) fn log_job_transition(
    stage: &'static str,
    article_id: &str,
    trace: &ArticleJobTrace,
    in_flight: usize,
) {
    event!(
        Level::INFO,
        article_id,
        job_kind = trace.job_kind,
        recovery_slug = trace.recovery_slug.as_deref().unwrap_or(""),
        stage,
        in_flight,
        "Article generation job state changed"
    );
}
