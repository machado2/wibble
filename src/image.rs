use std::env;
use std::path::PathBuf;
use std::{fs, io};

use image::ImageError;
use sea_orm::EntityTrait;

use crate::app_state::AppState;
use crate::auth::AuthUser;
use crate::content::can_view_article;
use crate::entities::prelude::*;
use crate::error::{Error, Result};
use crate::image_jobs::spawn_image_generation;
use crate::image_status::{is_pending_status, IMAGE_STATUS_FAILED};
use crate::s3;

pub struct ImagePayload {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
    pub cache_control: &'static str,
}

async fn read_stored_image(id: &str) -> Result<Vec<u8>> {
    let storage_type = env::var("STORAGE_TYPE").unwrap_or_else(|_| "local".to_string());
    if storage_type.eq_ignore_ascii_case("s3") {
        s3::download_image(id).await
    } else {
        let images_dir = env::var("IMAGES_DIR").expect("IMAGES_DIR is not set");
        let image_path = PathBuf::from(images_dir).join(format!("{}.jpg", id));
        fs::read(&image_path).map_err(|e| Error::Image(ImageError::IoError(e)))
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn placeholder_svg(status: &str, alt_text: &str) -> Vec<u8> {
    let (title, subtitle, accent) = if status == IMAGE_STATUS_FAILED {
        (
            "Image unavailable",
            "Generation failed or file is missing",
            "#9f3a38",
        )
    } else {
        (
            "Generating image",
            "Placeholder shown until the render finishes",
            "#7a5c17",
        )
    };
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 675" role="img" aria-label="{title}">
<rect width="1200" height="675" fill="#f3efe4"/>
<rect x="36" y="36" width="1128" height="603" rx="28" fill="#fffaf0" stroke="{accent}" stroke-width="4"/>
<circle cx="120" cy="120" r="18" fill="{accent}" opacity="0.85"/>
<text x="160" y="128" fill="#2e2a22" font-size="44" font-family="Georgia, serif">{title}</text>
<text x="80" y="220" fill="#5f584c" font-size="26" font-family="Georgia, serif">{subtitle}</text>
<foreignObject x="80" y="280" width="1040" height="250">
  <div xmlns="http://www.w3.org/1999/xhtml" style="font-family: Georgia, serif; font-size: 30px; color: #2e2a22; line-height: 1.35;">
    {alt_text}
  </div>
</foreignObject>
</svg>"##,
        title = escape_xml(title),
        subtitle = escape_xml(subtitle),
        accent = accent,
        alt_text = escape_xml(alt_text),
    )
    .into_bytes()
}

pub async fn get_image(
    state: &AppState,
    id: &str,
    auth_user: Option<&AuthUser>,
) -> Result<ImagePayload> {
    let (image, article) = ContentImage::find_by_id(id.to_string())
        .find_also_related(Content)
        .one(&state.db)
        .await
        .map_err(|e| Error::Database(format!("Database error reading image {}: {}", id, e)))?
        .ok_or_else(|| {
            Error::Image(ImageError::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Image {} not found", id),
            )))
        })?;
    let article = article.ok_or_else(|| {
        Error::Image(ImageError::IoError(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Article for image {} not found", id),
        )))
    })?;
    if !can_view_article(auth_user, &article) {
        return Err(Error::NotFound(Some(format!("Image {} not found", id))));
    }

    if let Ok(bytes) = read_stored_image(id).await {
        return Ok(ImagePayload {
            bytes,
            content_type: "image/jpeg",
            cache_control: "public, max-age=31536000, immutable",
        });
    }

    if is_pending_status(&image.status) && !state.is_image_generation_active(id).await {
        spawn_image_generation(state.clone(), id.to_string());
    }
    let placeholder_status = if is_pending_status(&image.status) {
        image.status.as_str()
    } else {
        IMAGE_STATUS_FAILED
    };

    Ok(ImagePayload {
        bytes: placeholder_svg(placeholder_status, &image.alt_text),
        content_type: "image/svg+xml",
        cache_control: "no-store",
    })
}
