use axum::extract::Multipart;
use axum::response::{Html, Redirect};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};

use crate::article_id::normalize_content_model;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::entities::{content as content_entity, content_image, prelude::*};
use crate::error::Error;
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::repositories::images::store_image_file;
use crate::services::article_translations::owned_article_source_text;
use crate::services::editorial_policy::enforce_article_output_policy;
use crate::translation_jobs::refresh_article_translations_after_edit;
use crate::wibble_request::WibbleRequest;

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

    Ok(Redirect::to(&format!("/content/{}", slug)))
}

pub(super) async fn render_edit_page(
    wr: WibbleRequest,
    slug: &str,
    agent_edit_max_length: usize,
) -> Result<Html<String>, Error> {
    let (_auth_user, article) = require_editable_article(&wr, slug).await?;
    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(&article.id))
        .all(&wr.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading images: {}", e)))?;

    let image_data: Vec<_> = images
        .iter()
        .map(|img| {
            serde_json::json!({
                "id": img.id,
                "alt_text": img.alt_text,
                "prompt": img.prompt,
            })
        })
        .collect();

    wr.template("edit")
        .await
        .insert("title", &format!("Edit: {}", article.title))
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
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "image" {
            let data = field
                .bytes()
                .await
                .map_err(|e| Error::Auth(format!("Failed to read upload: {}", e)))?;
            image_data = Some(data.to_vec());
        }
    }

    let image_data = image_data.ok_or_else(|| Error::Auth("No image uploaded".to_string()))?;
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

    Ok(Redirect::to(&format!("/content/{}/edit", slug)))
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

    Ok(Redirect::to(&format!("/content/{}", slug)))
}
