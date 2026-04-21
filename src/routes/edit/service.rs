use axum::extract::{multipart::MultipartError, Multipart};
use axum::http::StatusCode;
use axum::response::{Html, Redirect};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

use crate::article_id::normalize_content_model;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::entities::{content as content_entity, content_image, prelude::*};
use crate::error::Error;
use crate::image_jobs::spawn_image_generation;
use crate::image_status::{is_pending_status, IMAGE_STATUS_PENDING};
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::repositories::images::{normalize_uploaded_image, store_image_file};
use crate::services::article_translations::owned_article_source_text;
use crate::services::editorial_policy::enforce_article_output_policy;
use crate::translation_jobs::refresh_article_translations_after_edit;
use crate::wibble_request::WibbleRequest;

use super::MAX_IMAGE_UPLOAD_BYTES;

pub(super) async fn require_editable_article(
    wr: &WibbleRequest,
    slug: &str,
) -> Result<(AuthUser, content_entity::Model), Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?
        .clone();
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(slug))
        .one(&wr.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .map(normalize_content_model)
        .ok_or_else(|| Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(&auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    Ok((auth_user, article))
}

pub(super) async fn apply_article_edit(
    wr: &WibbleRequest,
    auth_user: &AuthUser,
    slug: &str,
    article: content_entity::Model,
    data: &super::EditArticleData,
    audit_action: &str,
    audit_details: Option<String>,
) -> Result<Redirect, Error> {
    let db = &wr.state.db;
    enforce_article_output_policy(&data.title, &data.description, &data.markdown)?;
    let previous_source = owned_article_source_text(&article);
    let translatable_content_changed = article.title != data.title
        || article.description != data.description
        || article.markdown.as_deref().unwrap_or("") != data.markdown;
    let article_id = article.id.clone();

    let mut active: content_entity::ActiveModel = article.into();
    active.title = ActiveValue::set(data.title.clone());
    active.description = ActiveValue::set(data.description.clone());
    active.markdown = ActiveValue::set(Some(data.markdown.clone()));
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating article: {}", e)))?;

    log_audit(db, auth_user, audit_action, "content", slug, audit_details).await?;
    if translatable_content_changed {
        if let Some(previous_source) = previous_source {
            refresh_article_translations_after_edit(
                wr.state.clone(),
                auth_user,
                slug,
                previous_source,
                crate::services::article_translations::OwnedArticleSourceText {
                    article_id,
                    title: data.title.clone(),
                    description: data.description.clone(),
                    markdown: data.markdown.clone(),
                },
            )
            .await?;
        }
    }

    Ok(Redirect::to(
        &wr.localized_path(&format!("/content/{}", slug)),
    ))
}

pub(super) async fn render_edit_page(
    wr: WibbleRequest,
    slug: &str,
    agent_edit_max_length: usize,
) -> Result<Html<String>, Error> {
    let text = wr.site_text();
    let (_auth_user, article) = require_editable_article(&wr, slug).await?;
    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(&article.id))
        .all(&wr.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading images: {}", e)))?;

    let image_data: Vec<_> = images
        .iter()
        .map(|img| {
            let is_generating = is_pending_status(&img.status);
            serde_json::json!({
                "id": img.id,
                "alt_text": img.alt_text,
                "prompt": img.prompt,
                "status": img.status,
                "status_label": text.image_status_label(&img.status),
                "status_note": text.image_status_note(&img.status),
                "is_generating": is_generating,
                "can_regenerate": !is_generating,
                "last_error": img.last_error,
            })
        })
        .collect();
    let max_chars_note = wr.site_text().template_strings()["edit"]["max_chars"]
        .as_str()
        .unwrap_or_default()
        .replace("%MAX_CHARS%", &agent_edit_max_length.to_string());

    wr.template("edit")
        .await
        .insert("title", &text.edit_meta_title(&article.title))
        .insert("robots", "noindex,nofollow")
        .insert("article_title", &article.title)
        .insert("article_description", &article.description)
        .insert(
            "article_markdown",
            article.markdown.as_deref().unwrap_or(""),
        )
        .insert("slug", slug)
        .insert("id", &article.id)
        .insert("images", &image_data)
        .insert("agent_edit_max_length", &agent_edit_max_length)
        .insert("max_chars_note", &max_chars_note)
        .render()
}

pub(super) async fn replace_article_image(
    wr: WibbleRequest,
    slug: &str,
    image_id: &str,
    mut multipart: Multipart,
) -> Result<Redirect, Error> {
    let (auth_user, article) = require_editable_article(&wr, slug).await?;
    let db = &wr.state.db;

    let img = ContentImage::find_by_id(image_id.to_string())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding image: {}", e)))?
        .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;

    if img.content_id != article.id {
        return Err(Error::Auth(
            "Image does not belong to this article".to_string(),
        ));
    }

    let mut image_data = None;
    while let Some(field) = multipart.next_field().await.map_err(map_multipart_error)? {
        if field.name().unwrap_or("") == "image" {
            let data = field.bytes().await.map_err(map_multipart_error)?;
            image_data = Some(normalize_uploaded_image(data.as_ref())?);
        }
    }

    let image_data =
        image_data.ok_or_else(|| Error::BadRequest("No image uploaded".to_string()))?;
    store_image_file(image_id, image_data).await?;

    log_audit(
        db,
        &auth_user,
        "replace_image",
        "content_image",
        image_id,
        Some(format!("article={}", slug)),
    )
    .await?;

    Ok(Redirect::to(
        &wr.localized_path(&format!("/content/{}/edit#images", slug)),
    ))
}

pub(super) async fn regenerate_article_image(
    wr: WibbleRequest,
    slug: &str,
    image_id: &str,
) -> Result<Redirect, Error> {
    let (auth_user, article) = require_editable_article(&wr, slug).await?;
    let db = &wr.state.db;

    let image = ContentImage::find_by_id(image_id.to_string())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding image: {}", e)))?
        .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;

    if image.content_id != article.id {
        return Err(Error::Auth(
            "Image does not belong to this article".to_string(),
        ));
    }

    if wr.state.is_image_generation_active(image_id).await {
        return Ok(Redirect::to(
            &wr.localized_path(&format!("/content/{}/edit#images", slug)),
        ));
    }

    mark_image_pending_for_regeneration(db, image).await?;
    spawn_image_generation(wr.state.clone(), image_id.to_string());

    log_audit(
        db,
        &auth_user,
        "regenerate_image",
        "content_image",
        image_id,
        Some(format!("article={}", slug)),
    )
    .await?;

    Ok(Redirect::to(
        &wr.localized_path(&format!("/content/{}/edit#images", slug)),
    ))
}

fn map_multipart_error(err: MultipartError) -> Error {
    match err.status() {
        StatusCode::PAYLOAD_TOO_LARGE => Error::BadRequest(format!(
            "Uploaded image is too large. Maximum size is {} MB.",
            MAX_IMAGE_UPLOAD_BYTES / (1024 * 1024)
        )),
        _ => Error::BadRequest(format!("Failed to read upload: {}", err.body_text())),
    }
}

pub(super) async fn mark_image_pending_for_regeneration(
    db: &sea_orm::DatabaseConnection,
    image: content_image::Model,
) -> Result<(), Error> {
    let mut active = content_image::ActiveModel::from(image);
    active.status = ActiveValue::set(IMAGE_STATUS_PENDING.to_string());
    active.last_error = ActiveValue::set(None);
    active.parameters = ActiveValue::set(None);
    active.generation_started_at = ActiveValue::set(None);
    active.generation_finished_at = ActiveValue::set(None);
    active.provider_job_id = ActiveValue::set(None);
    active.provider_job_url = ActiveValue::set(None);
    active.regenerate = ActiveValue::set(true);
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error queueing image regeneration: {}", e)))?;
    Ok(())
}

pub(super) async fn toggle_publish(wr: WibbleRequest, slug: &str) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .map(normalize_content_model)
        .ok_or_else(|| Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_toggle_publish(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to toggle publish state".to_string(),
        ));
    }

    let new_state = !article.published;
    let mut active: content_entity::ActiveModel = article.into();
    active.published = ActiveValue::set(new_state);
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating publish state: {}", e)))?;

    log_audit(
        db,
        auth_user,
        if new_state {
            "publish_article"
        } else {
            "unpublish_article"
        },
        "content",
        slug,
        None,
    )
    .await?;

    Ok(Redirect::to(
        &wr.localized_path(&format!("/content/{}", slug)),
    ))
}
