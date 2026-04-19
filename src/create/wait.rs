use axum::response::Html;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;

use crate::app_state::AppState;
use crate::article_id::normalize_optional_content_model;
use crate::create::clarify::parse_clarification_request;
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
    clarification_question: Option<String>,
    clarification_deadline: Option<String>,
    phase_items: Vec<WaitPhaseItem>,
}

#[derive(Serialize)]
struct WaitPhaseItem {
    label: String,
    state: String,
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
            "The draft is paused because the brief is still ambiguous enough to change the article materially.".to_string(),
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

fn build_wait_phase_items(
    phase: Option<&str>,
    clarification_requested: bool,
) -> Vec<WaitPhaseItem> {
    let steps = [
        (ARTICLE_JOB_PHASE_QUEUED, "Queued"),
        (ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, "Clarify"),
        (ARTICLE_JOB_PHASE_WRITING, "Write"),
        (ARTICLE_JOB_PHASE_RENDERING_IMAGES, "Images"),
        (ARTICLE_JOB_PHASE_READY_FOR_REVIEW, "Review"),
    ];

    let phase_rank = match phase.unwrap_or(ARTICLE_JOB_PHASE_QUEUED) {
        ARTICLE_JOB_PHASE_AWAITING_USER_INPUT => 1,
        ARTICLE_JOB_PHASE_WRITING
        | ARTICLE_JOB_PHASE_RESEARCHING
        | ARTICLE_JOB_PHASE_TRANSLATING => {
            if clarification_requested {
                2
            } else {
                1
            }
        }
        ARTICLE_JOB_PHASE_RENDERING_IMAGES => {
            if clarification_requested {
                3
            } else {
                2
            }
        }
        ARTICLE_JOB_PHASE_READY_FOR_REVIEW => {
            if clarification_requested {
                4
            } else {
                3
            }
        }
        _ => 0,
    };

    steps
        .into_iter()
        .filter(|(step_phase, _)| {
            clarification_requested || *step_phase != ARTICLE_JOB_PHASE_AWAITING_USER_INPUT
        })
        .map(|(_step_phase, label)| {
            let step_rank = match label {
                "Queued" => 0,
                "Clarify" => 1,
                "Write" => {
                    if clarification_requested {
                        2
                    } else {
                        1
                    }
                }
                "Images" => {
                    if clarification_requested {
                        3
                    } else {
                        2
                    }
                }
                "Review" => {
                    if clarification_requested {
                        4
                    } else {
                        3
                    }
                }
                _ => 0,
            };
            let state = if phase_rank > step_rank {
                "done"
            } else if phase_rank == step_rank {
                "active"
            } else {
                "pending"
            };
            WaitPhaseItem {
                label: label.to_string(),
                state: state.to_string(),
            }
        })
        .collect()
}

async fn build_wait_summary(
    state: &AppState,
    id: &str,
    is_logged_in: bool,
    job_phase: Option<&str>,
    job_preview_payload: Option<&str>,
) -> Result<WaitSummary, Error> {
    let fallback_publication_copy = publication_copy(is_logged_in);
    let clarification = parse_clarification_request(job_preview_payload);
    let clarification_question = clarification.as_ref().map(|value| value.question.clone());
    let clarification_deadline = clarification
        .as_ref()
        .and_then(|value| value.formatted_deadline());
    let phase_items = build_wait_phase_items(job_phase, clarification_question.is_some());
    let article = Content::find()
        .filter(content::Column::Id.eq(id))
        .one(&state.db)
        .await
        .map(normalize_optional_content_model)
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
            clarification_question: None,
            clarification_deadline: None,
            phase_items,
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
            clarification_question,
            clarification_deadline,
            phase_items,
        })
    }
}

pub async fn render_wait_page(wr: &WibbleRequest, id: &str) -> Result<Html<String>, Error> {
    let job = ArticleJobService::new(wr.state.clone())
        .load_job(id)
        .await?;
    let wait_summary = build_wait_summary(
        &wr.state,
        id,
        wr.auth_user.is_some(),
        job.as_ref().map(|job| job.phase.as_str()),
        job.as_ref().and_then(|job| job.preview_payload.as_deref()),
    )
    .await?;
    wr.template("wait")
        .await
        .insert("id", id)
        .insert("title", "Generating article")
        .insert(
            "description",
            "The article is still being generated and this page auto-refreshes.",
        )
        .insert("robots", "noindex,nofollow")
        .insert(
            "wait_auto_refresh",
            &wait_summary.clarification_question.is_none(),
        )
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

#[cfg(test)]
mod tests {
    use super::{build_wait_phase_items, queued_stage_copy};
    use crate::services::article_jobs::{
        ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, ARTICLE_JOB_PHASE_RESEARCHING,
    };

    #[test]
    fn wait_phase_items_include_clarify_step_when_question_is_pending() {
        let items = build_wait_phase_items(Some(ARTICLE_JOB_PHASE_AWAITING_USER_INPUT), true);

        assert_eq!(items.len(), 5);
        assert_eq!(items[1].label, "Clarify");
        assert_eq!(items[1].state, "active");
    }

    #[test]
    fn queued_stage_copy_describes_research_phase() {
        let (title, description) = queued_stage_copy(Some(ARTICLE_JOB_PHASE_RESEARCHING));

        assert!(title.contains("Researching"));
        assert!(description.contains("bounded context"));
    }
}
