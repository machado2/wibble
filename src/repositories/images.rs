use std::path::PathBuf;
use std::{env, fs};

use sea_orm::{ConnectionTrait, EntityTrait};

use crate::entities::{content_image, prelude::*};
use crate::error::Error;
use crate::image_status::IMAGE_STATUS_COMPLETED;
use crate::s3;

pub async fn store_image_file(id: &str, img: Vec<u8>) -> Result<(), Error> {
    let storage_type = env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    if storage_type.eq_ignore_ascii_case("s3") {
        s3::upload_image(id, img).await?;
    } else {
        let images_dir = env::var("IMAGES_DIR").expect("IMAGES_DIR is not set");
        let image_path = PathBuf::from(images_dir).join(format!("{}.jpg", id));
        if let Some(parent) = image_path.parent() {
            fs::create_dir_all(parent).map_err(|e| Error::Image(image::ImageError::IoError(e)))?;
        }
        fs::write(&image_path, img).map_err(|e| Error::Image(image::ImageError::IoError(e)))?;
    }
    Ok(())
}

pub(super) async fn save_generated_image(
    article_id: String,
    prompt: String,
    alt_text: String,
    parameters: String,
    id: String,
    db: &impl ConnectionTrait,
    img: Vec<u8>,
) -> Result<(), Error> {
    let now = chrono::Utc::now().naive_local();
    let content_image = content_image::Model {
        id: id.clone(),
        content_id: article_id,
        prompt: prompt.clone(),
        alt_text,
        created_at: now,
        model: None,
        fail_count: 0,
        flagged: false,
        generator: None,
        parameters: Some(parameters),
        prompt_hash: None,
        regenerate: false,
        seed: None,
        view_count: 0,
        status: IMAGE_STATUS_COMPLETED.to_string(),
        last_error: None,
        generation_started_at: Some(now),
        generation_finished_at: Some(now),
        provider_job_id: None,
        provider_job_url: None,
    };
    ContentImage::insert(content_image::ActiveModel::from(content_image))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting content_image: {}", e)))?;
    store_image_file(&id, img).await?;
    Ok(())
}
