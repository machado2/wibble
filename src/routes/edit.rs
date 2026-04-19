use axum::extract::{Multipart, Path};
use axum::response::{Html, Redirect};
use axum::routing::post;
use axum::{Form, Router};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::entities::{content as content_entity, content_image, prelude::*};
use crate::error::Error;
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::repository::store_image_file;
use crate::wibble_request::WibbleRequest;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/content/{slug}/edit",
            axum::routing::get(get_edit_article).post(post_edit_article),
        )
        .route(
            "/content/{slug}/images/{image_id}",
            post(post_replace_image),
        )
        .route("/content/{slug}/publish", post(post_toggle_publish))
}

#[derive(Deserialize, Debug)]
struct EditArticleData {
    title: String,
    description: String,
    markdown: String,
}

async fn get_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Html<String>, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(&article.id))
        .all(db)
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
        .insert("slug", &slug)
        .insert("id", &article.id)
        .insert("images", &image_data)
        .render()
}

async fn post_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<EditArticleData>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    let mut active: content_entity::ActiveModel = article.into();
    active.title = ActiveValue::set(data.title.clone());
    active.description = ActiveValue::set(data.description);
    active.markdown = ActiveValue::set(Some(data.markdown.clone()));
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating article: {}", e)))?;

    log_audit(db, auth_user, "edit_article", "content", &slug, None).await?;

    Ok(Redirect::to(&format!("/content/{}", slug)))
}

async fn post_replace_image(
    wr: WibbleRequest,
    Path((slug, image_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;

    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(auth_user, &article) {
        return Err(Error::Auth("Not authorized".to_string()));
    }

    let img = ContentImage::find_by_id(image_id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding image: {}", e)))?
        .ok_or(Error::NotFound(Some(format!(
            "Image {} not found",
            image_id
        ))))?;

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
    store_image_file(&image_id, image_data).await?;

    log_audit(
        db,
        auth_user,
        "replace_image",
        "content_image",
        &image_id,
        Some(format!("article={}", slug)),
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}/edit", slug)))
}

async fn post_toggle_publish(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or(Error::NotFound(Some(format!("Article {} not found", slug))))?;

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
        &slug,
        None,
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}", slug)))
}
