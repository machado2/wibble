use std::env;
use std::time::{Duration, Instant};

use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::entities::{content_image, prelude::*};
use crate::error::Error;
use crate::image_generator::replicate::ReplicatePrediction;
use crate::image_status::{
    is_pending_status, IMAGE_STATUS_COMPLETED, IMAGE_STATUS_FAILED, IMAGE_STATUS_PENDING,
    IMAGE_STATUS_PROCESSING,
};
use crate::repository::store_image_file;

fn pending_resume_interval_seconds() -> u64 {
    env::var("IMAGE_RESUME_INTERVAL_SECONDS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(30)
}

fn pending_resume_batch_size() -> u64 {
    env::var("IMAGE_RESUME_BATCH_SIZE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(100)
}

fn replicate_parameters(prompt: &str) -> String {
    serde_json::json!({
        "input": { "prompt": prompt }
    })
    .to_string()
}

async fn load_content_image(
    state: &AppState,
    image_id: &str,
) -> Result<Option<content_image::Model>, Error> {
    ContentImage::find_by_id(image_id.to_string())
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading content image {}: {}", image_id, e)))
}

async fn mark_processing(
    state: &AppState,
    image: content_image::Model,
    provider_job_id: Option<String>,
    provider_job_url: Option<String>,
) -> Result<content_image::Model, Error> {
    let mut active = content_image::ActiveModel::from(image);
    active.status = ActiveValue::set(IMAGE_STATUS_PROCESSING.to_string());
    active.last_error = ActiveValue::set(None);
    active.generation_started_at = ActiveValue::set(Some(chrono::Utc::now().naive_local()));
    active.generation_finished_at = ActiveValue::set(None);
    active.provider_job_id = ActiveValue::set(provider_job_id);
    active.provider_job_url = ActiveValue::set(provider_job_url);
    active
        .update(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error marking image as processing: {}", e)))
}

async fn mark_failed(
    state: &AppState,
    image: content_image::Model,
    err: &Error,
) -> Result<(), Error> {
    let mut active = content_image::ActiveModel::from(image.clone());
    active.status = ActiveValue::set(IMAGE_STATUS_FAILED.to_string());
    active.last_error = ActiveValue::set(Some(err.to_string()));
    active.fail_count = ActiveValue::set(image.fail_count + 1);
    active.generation_finished_at = ActiveValue::set(Some(chrono::Utc::now().naive_local()));
    active
        .update(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error marking image as failed: {}", e)))?;
    Ok(())
}

async fn mark_completed(
    state: &AppState,
    image: content_image::Model,
    parameters: String,
) -> Result<(), Error> {
    let mut active = content_image::ActiveModel::from(image);
    active.status = ActiveValue::set(IMAGE_STATUS_COMPLETED.to_string());
    active.last_error = ActiveValue::set(None);
    active.parameters = ActiveValue::set(Some(parameters));
    active.generation_finished_at = ActiveValue::set(Some(chrono::Utc::now().naive_local()));
    active
        .update(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error marking image as completed: {}", e)))?;
    Ok(())
}

async fn process_generic_image(state: &AppState, image: content_image::Model) -> Result<(), Error> {
    let image = mark_processing(state, image, None, None).await?;
    match state
        .image_generator
        .create_image(image.prompt.clone())
        .await
    {
        Ok(created) => {
            store_image_file(&image.id, created.data).await?;
            mark_completed(state, image, created.parameters).await
        }
        Err(err) => {
            mark_failed(state, image, &err).await?;
            Err(err)
        }
    }
}

async fn process_replicate_image(
    state: &AppState,
    image: content_image::Model,
) -> Result<(), Error> {
    let replicate = state
        .replicate_image_generator
        .as_ref()
        .ok_or_else(|| Error::ImageGeneration("Replicate generator is not configured".into()))?;
    let request_started = Instant::now();
    let _permit = replicate
        .acquire_generation_slot(image.prompt.len())
        .await?;
    let image_id = image.id.clone();

    let prediction = if let Some(poll_url) = image.provider_job_url.clone() {
        let provider_job_id = image.provider_job_id.clone();
        let image = mark_processing(state, image, provider_job_id, Some(poll_url.clone())).await?;
        ReplicatePrediction {
            id: image.provider_job_id.clone(),
            poll_url,
            parameters: replicate_parameters(&image.prompt),
        }
    } else {
        let prediction = replicate.create_prediction(&image.prompt).await?;
        let _ = mark_processing(
            state,
            image,
            prediction.id.clone(),
            Some(prediction.poll_url.clone()),
        )
        .await?;
        prediction
    };

    match replicate
        .await_prediction(
            prediction.id.as_deref(),
            &prediction.poll_url,
            prediction.parameters,
            request_started,
        )
        .await
    {
        Ok(created) => {
            let image = load_content_image(state, &image_id)
                .await?
                .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;
            store_image_file(&image.id, created.data).await?;
            mark_completed(state, image, created.parameters).await
        }
        Err(err) => {
            let image = load_content_image(state, &image_id)
                .await?
                .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;
            mark_failed(state, image, &err).await?;
            Err(err)
        }
    }
}

async fn process_image_generation(state: &AppState, image_id: &str) -> Result<(), Error> {
    let image = load_content_image(state, image_id)
        .await?
        .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;
    if image.status == IMAGE_STATUS_COMPLETED || image.status == IMAGE_STATUS_FAILED {
        return Ok(());
    }
    if state.replicate_image_generator.is_some() {
        process_replicate_image(state, image).await
    } else {
        process_generic_image(state, image).await
    }
}

pub async fn spawn_image_generation(state: AppState, image_id: String) {
    if !state.try_mark_image_generation_started(&image_id).await {
        return;
    }

    tokio::spawn(async move {
        let result = process_image_generation(&state, &image_id).await;
        if let Err(err) = result {
            event!(
                Level::ERROR,
                image_id = %image_id,
                error = %err,
                "Async image generation failed"
            );
        } else {
            event!(Level::INFO, image_id = %image_id, "Async image generation finished");
        }
        state.mark_image_generation_finished(&image_id).await;
    });
}

pub async fn enqueue_pending_images(state: AppState, image_ids: Vec<String>) {
    for image_id in image_ids {
        spawn_image_generation(state.clone(), image_id).await;
    }
}

pub async fn resume_pending_images(state: &AppState) -> Result<(), Error> {
    let images = ContentImage::find()
        .filter(
            content_image::Column::Status.is_in([IMAGE_STATUS_PENDING, IMAGE_STATUS_PROCESSING]),
        )
        .order_by_asc(content_image::Column::CreatedAt)
        .limit(pending_resume_batch_size())
        .all(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading pending images: {}", e)))?;

    for image in images {
        if is_pending_status(&image.status) {
            spawn_image_generation(state.clone(), image.id).await;
        }
    }
    Ok(())
}

pub fn spawn_resume_loop(state: AppState) {
    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(Duration::from_secs(pending_resume_interval_seconds()));
        loop {
            interval.tick().await;
            if let Err(err) = resume_pending_images(&state).await {
                event!(Level::ERROR, error = %err, "Failed to resume pending images");
            }
        }
    });
}
