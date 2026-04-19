use axum::response::Html;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::app_state::AppState;
use crate::entities::prelude::*;
use crate::entities::{content, content_image};
use crate::error::Error;
use crate::image_status::{
    IMAGE_STATUS_COMPLETED, IMAGE_STATUS_FAILED, IMAGE_STATUS_PENDING, IMAGE_STATUS_PROCESSING,
};
use crate::services::article_jobs::{
    is_in_progress_job_status, ArticleJobService, ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
    ARTICLE_JOB_PHASE_QUEUED, ARTICLE_JOB_PHASE_READY_FOR_REVIEW,
    ARTICLE_JOB_PHASE_RENDERING_IMAGES, ARTICLE_JOB_PHASE_RESEARCHING,
    ARTICLE_JOB_PHASE_TRANSLATING, ARTICLE_JOB_PHASE_WRITING, ARTICLE_JOB_STATUS_COMPLETED,
    ARTICLE_JOB_STATUS_FAILED,
};
use crate::wibble_request::WibbleRequest;

#[derive(Serialize)]
struct WaitSummary {
    article_title: Option<String>,
    slug: Option<String>,
    stage_title: String,
    stage_description: String,
    publication_title: String,
    publication_note: String,
    image_total: usize,
    image_completed: usize,
    image_processing: usize,
    image_failed: usize,
}

#[allow(clippy::large_enum_variant)]
pub enum WaitResponse {
    Redirect(String),
    Html(Html<String>),
    InternalError,
    NotFound,
}

fn publication_copy(is_logged_in: bool) -> (String, String) {
    if is_logged_in {
        (
            "Destination: draft".to_string(),
            "Signed-in articles stay private until you review and publish them.".to_string(),
        )
    } else {
        (
            "Destination: public".to_string(),
            "Anonymous articles publish immediately and are not tied to an editable owner account."
                .to_string(),
        )
    }
}

fn queued_stage_copy(phase: Option<&str>) -> (String, String) {
    match phase.unwrap_or(ARTICLE_JOB_PHASE_WRITING) {
        ARTICLE_JOB_PHASE_QUEUED => (
            "Queued for generation".to_string(),
            "The prompt is waiting for a generation slot before drafting starts.".to_string(),
        ),
        ARTICLE_JOB_PHASE_RESEARCHING => (
            "Researching the brief".to_string(),
            "The job is gathering bounded context before the draft is written.".to_string(),
        ),
        ARTICLE_JOB_PHASE_TRANSLATING => (
            "Translating the draft".to_string(),
            "The article text is being transformed into a new language variant.".to_string(),
        ),
        ARTICLE_JOB_PHASE_AWAITING_USER_INPUT => (
            "Waiting for clarification".to_string(),
            "The draft is paused until the missing instruction is resolved.".to_string(),
        ),
        ARTICLE_JOB_PHASE_READY_FOR_REVIEW => (
            "Preparing review".to_string(),
            "The draft is being packaged for a final review pass.".to_string(),
        ),
        ARTICLE_JOB_PHASE_RENDERING_IMAGES => (
            "Rendering illustrations".to_string(),
            "The story draft is ready and the image queue is actively rendering art.".to_string(),
        ),
        _ => (
            "Drafting the story".to_string(),
            "The headline, angle, and article body are still being assembled.".to_string(),
        ),
    }
}

async fn build_wait_summary(
    state: &AppState,
    id: &str,
    is_logged_in: bool,
    job_phase: Option<&str>,
) -> Result<WaitSummary, Error> {
    let fallback_publication_copy = publication_copy(is_logged_in);
    let article = Content::find()
        .filter(content::Column::Id.eq(id))
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading article wait state: {}", e)))?;

    if let Some(article) = article {
        let images = ContentImage::find()
            .filter(content_image::Column::ContentId.eq(article.id.clone()))
            .all(&state.db)
            .await
            .map_err(|e| Error::Database(format!("Error loading image wait state: {}", e)))?;
        let image_total = images.len();
        let image_completed = images
            .iter()
            .filter(|img| img.status == IMAGE_STATUS_COMPLETED)
            .count();
        let image_processing = images
            .iter()
            .filter(|img| {
                img.status == IMAGE_STATUS_PROCESSING || img.status == IMAGE_STATUS_PENDING
            })
            .count();
        let image_failed = images
            .iter()
            .filter(|img| img.status == IMAGE_STATUS_FAILED)
            .count();
        let (stage_title, stage_description) = if image_total == 0 && article.markdown.is_none() {
            (
                "Drafting the story".to_string(),
                "The headline, angle, and article body are still being assembled.".to_string(),
            )
        } else if image_total == 0 {
            (
                "Preparing the article".to_string(),
                "The story draft is ready and the page is being finalized.".to_string(),
            )
        } else if image_processing > 0 {
            (
                "Rendering illustrations".to_string(),
                "The story is ready and the image queue is actively rendering art.".to_string(),
            )
        } else if image_failed > 0 && image_completed < image_total {
            (
                "Recovering the image set".to_string(),
                "Some illustrations failed and the article is waiting on the remaining results."
                    .to_string(),
            )
        } else {
            (
                "Finalizing the article".to_string(),
                "The draft is complete and the page is about to go live.".to_string(),
            )
        };

        let (publication_title, publication_note) =
            publication_copy(article.author_email.is_some());
        Ok(WaitSummary {
            article_title: Some(article.title),
            slug: Some(article.slug),
            stage_title,
            stage_description,
            publication_title,
            publication_note,
            image_total,
            image_completed,
            image_processing,
            image_failed,
        })
    } else {
        let (stage_title, stage_description) = queued_stage_copy(job_phase);
        Ok(WaitSummary {
            article_title: None,
            slug: None,
            stage_title,
            stage_description,
            publication_title: fallback_publication_copy.0,
            publication_note: fallback_publication_copy.1,
            image_total: 0,
            image_completed: 0,
            image_processing: 0,
            image_failed: 0,
        })
    }
}

pub async fn render_wait_page(wr: &WibbleRequest, id: &str) -> Result<Html<String>, Error> {
    let job_phase = ArticleJobService::new(wr.state.clone())
        .load_job(id)
        .await?
        .map(|job| job.phase);
    let wait_summary =
        build_wait_summary(&wr.state, id, wr.auth_user.is_some(), job_phase.as_deref()).await?;
    wr.template("wait")
        .await
        .insert("id", id)
        .insert("title", "Generating article")
        .insert(
            "description",
            "The article is still being generated and this page auto-refreshes.",
        )
        .insert("robots", "noindex,nofollow")
        .insert("wait_summary", &wait_summary)
        .render()
}

pub async fn wait(wr: WibbleRequest, id: &str) -> WaitResponse {
    let job_service = ArticleJobService::new(wr.state.clone());
    let job = job_service.ensure_job_progress(id).await;
    match job {
        Ok(Some(job)) if job.status == ARTICLE_JOB_STATUS_COMPLETED => {
            let content = Content::find()
                .filter(content::Column::Id.eq(id))
                .one(&wr.state.db)
                .await;
            match content {
                Ok(Some(content)) => WaitResponse::Redirect(content.slug),
                Ok(None) => WaitResponse::NotFound,
                Err(_) => WaitResponse::InternalError,
            }
        }
        Ok(Some(job)) if job.status == ARTICLE_JOB_STATUS_FAILED => WaitResponse::InternalError,
        Ok(Some(job)) if is_in_progress_job_status(&job.status) => {
            match render_wait_page(&wr, id).await {
                Ok(html) => WaitResponse::Html(html),
                Err(_) => WaitResponse::InternalError,
            }
        }
        Ok(Some(_)) => WaitResponse::InternalError,
        Ok(None) => WaitResponse::NotFound,
        Err(_) => WaitResponse::InternalError,
    }
}
