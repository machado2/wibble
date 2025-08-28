use std::{env, path::PathBuf};
use std::fs;
use sea_orm::prelude::*;

use crate::error::{Error, Result};
use crate::s3;
use image;

pub async fn get_image(_db: &DatabaseConnection, id: &str) -> Result<Vec<u8>> {
    let storage_type = env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    if storage_type.eq_ignore_ascii_case("s3") {
        s3::download_image(id).await
    } else {
        let images_dir = env::var("IMAGES_DIR").expect("IMAGES_DIR is not set");
        let image_path = PathBuf::from(images_dir).join(format!("{}.jpg", id));
        fs::read(&image_path)
            .map_err(|e| Error::Image(image::ImageError::IoError(e)))
    }
}
