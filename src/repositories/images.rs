use std::path::PathBuf;
use std::{env, fs};

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use sea_orm::{ConnectionTrait, EntityTrait};

use crate::entities::{content_image, prelude::*};
use crate::error::Error;
use crate::image_status::IMAGE_STATUS_COMPLETED;
use crate::s3;

pub fn normalize_uploaded_image(img: &[u8]) -> Result<Vec<u8>, Error> {
    let format = image::guess_format(img).map_err(|_| {
        Error::BadRequest("Unsupported image format. Upload a JPG, JPEG, or PNG image.".into())
    })?;
    if !matches!(format, ImageFormat::Jpeg | ImageFormat::Png) {
        return Err(Error::BadRequest(
            "Unsupported image format. Upload a JPG, JPEG, or PNG image.".into(),
        ));
    }

    let decoded = image::load_from_memory_with_format(img, format).map_err(|_| {
        Error::BadRequest("Invalid image file. Upload a valid JPG, JPEG, or PNG image.".into())
    })?;
    let flattened = flatten_image_for_jpeg(decoded);

    let mut output = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut output, 90);
    encoder
        .encode_image(&DynamicImage::ImageRgb8(flattened))
        .map_err(Error::Image)?;

    Ok(output)
}

fn flatten_image_for_jpeg(image: DynamicImage) -> RgbImage {
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut rgb = RgbImage::new(width, height);

    for (x, y, pixel) in rgba.enumerate_pixels() {
        let [red, green, blue, alpha] = pixel.0;
        let alpha = u16::from(alpha);
        let blend = |channel: u8| -> u8 {
            (((u16::from(channel) * alpha) + (255 * (255 - alpha)) + 127) / 255) as u8
        };

        rgb.put_pixel(x, y, Rgb([blend(red), blend(green), blend(blue)]));
    }

    rgb
}

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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

    use crate::error::Error;

    use super::normalize_uploaded_image;

    fn encode_test_image(format: ImageFormat) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(2, 2, Rgba([16, 32, 64, 128])));
        let mut bytes = Cursor::new(Vec::new());
        image.write_to(&mut bytes, format).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn normalize_uploaded_image_accepts_png_and_reencodes_to_jpeg() {
        let png = encode_test_image(ImageFormat::Png);

        let normalized = normalize_uploaded_image(&png).unwrap();

        assert_eq!(image::guess_format(&normalized).unwrap(), ImageFormat::Jpeg);
    }

    #[test]
    fn normalize_uploaded_image_rejects_unsupported_formats() {
        let gif = encode_test_image(ImageFormat::Gif);

        let err = normalize_uploaded_image(&gif).unwrap_err();

        assert!(matches!(err, Error::BadRequest(_)));
        assert!(err.to_string().contains("Upload a JPG, JPEG, or PNG image"));
    }
}
