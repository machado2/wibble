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
use crate::services::article_jobs::ArticleJobService;
use crate::tasklist::TaskResult;
use crate::wibble_request::WibbleRequest;

#[derive(Serialize)]
struct WaitSummary {
    article_title: Option<String>,
    slug: Option<String>,
    stage_title: String,
    stage_description: String,
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

async fn build_wait_summary(state: &AppState, id: &str) -> Result<WaitSummary, Error> {
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

        Ok(WaitSummary {
            article_title: Some(article.title),
            slug: Some(article.slug),
            stage_title,
            stage_description,
            image_total,
            image_completed,
            image_processing,
            image_failed,
        })
    } else {
        Ok(WaitSummary {
            article_title: None,
            slug: None,
            stage_title: "Drafting the story".to_string(),
            stage_description:
                "The prompt is in the queue and the article body is still being written."
                    .to_string(),
            image_total: 0,
            image_completed: 0,
            image_processing: 0,
            image_failed: 0,
        })
    }
}

pub async fn render_wait_page(wr: &WibbleRequest, id: &str) -> Result<Html<String>, Error> {
    let wait_summary = build_wait_summary(&wr.state, id).await?;
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
    let task = job_service.task_result(id).await;
    match task {
        Ok(TaskResult::Success) => {
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
        Ok(TaskResult::Error) => WaitResponse::InternalError,
        Ok(TaskResult::Processing) => match render_wait_page(&wr, id).await {
            Ok(html) => WaitResponse::Html(html),
            Err(_) => WaitResponse::InternalError,
        },
        _ => WaitResponse::NotFound,
    }
}
