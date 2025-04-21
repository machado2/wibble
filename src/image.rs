use std::{env, path::PathBuf};
use std::fs;
use sea_orm::prelude::*;

use crate::error::{Error, Result};
use image;

pub async fn get_image(_db: &DatabaseConnection, id: &str) -> Result<Vec<u8>> {
    let images_dir = env::var("IMAGES_DIR").expect("IMAGES_DIR is not set");
    let image_path = PathBuf::from(images_dir).join(format!("{}.jpg", id));
    fs::read(&image_path)
        .map_err(|e| Error::Image(image::ImageError::IoError(e)))
}
