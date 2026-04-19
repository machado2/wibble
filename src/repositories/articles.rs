use sea_orm::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait,
};

use crate::entities::{content, content_image, prelude::*};
use crate::error::Error;
use crate::image_generator::{ImageGenerated, ImageToCreate};
use crate::image_status::IMAGE_STATUS_PENDING;
use crate::services::article_persistence::{
    build_saved_content_model, next_slug_for_title, SavedContentInput,
};

use super::images::save_generated_image;

pub struct Article {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub prompt_version: i32,
    pub instructions: String,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub images: Vec<ImageGenerated>,
    pub author_email: Option<String>,
}

pub struct PendingArticle {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub prompt_version: i32,
    pub instructions: String,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub images: Vec<ImageToCreate>,
    pub image_generator: String,
    pub author_email: Option<String>,
}

pub async fn save_pending_article(
    db: &DatabaseConnection,
    article: PendingArticle,
) -> Result<(), Error> {
    let PendingArticle {
        id,
        title,
        markdown,
        prompt_version,
        instructions,
        start_time,
        model,
        description,
        images,
        image_generator,
        author_email,
    } = article;
    let existing = Content::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking existing article: {}", e)))?;
    let has_existing = existing.is_some();
    let slug = if let Some(existing) = &existing {
        existing.slug.clone()
    } else {
        next_slug_for_title(db, &title)
            .await
            .unwrap_or(id.to_string())
    };
    let now = chrono::Utc::now().naive_local();
    let recovered_from_dead_link = existing
        .as_ref()
        .map(|existing| existing.recovered_from_dead_link)
        .unwrap_or(false);
    let first_image_id = images
        .first()
        .ok_or(Error::ImageGeneration("No images planned".into()))?
        .id
        .clone();
    let c = build_saved_content_model(
        SavedContentInput {
            id: id.to_string(),
            slug,
            markdown: markdown.clone(),
            prompt_version,
            start_time,
            model,
            description,
            image_id: Some(first_image_id),
            title,
            instructions,
            author_email,
            recovered_from_dead_link,
        },
        now,
    );

    let mut c = content::ActiveModel::from(c);
    if has_existing {
        c = c.reset_all();
    }
    db.transaction(|tx| {
        Box::pin(async move {
            if has_existing {
                Content::update(c)
                    .exec(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error updating content: {}", e)))?;
                ContentImage::delete_many()
                    .filter(content_image::Column::ContentId.eq(id.clone()))
                    .exec(tx)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Error deleting content images: {}", e))
                    })?;
            } else {
                Content::insert(c)
                    .exec(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error inserting content: {}", e)))?;
            }

            for image in images {
                let pending_image = content_image::Model {
                    id: image.id,
                    content_id: id.clone(),
                    prompt_hash: None,
                    prompt: image.prompt,
                    alt_text: image.caption,
                    created_at: now,
                    flagged: false,
                    regenerate: false,
                    fail_count: 0,
                    generator: Some(image_generator.clone()),
                    model: None,
                    seed: None,
                    parameters: None,
                    view_count: 0,
                    status: IMAGE_STATUS_PENDING.to_string(),
                    last_error: None,
                    generation_started_at: None,
                    generation_finished_at: None,
                    provider_job_id: None,
                    provider_job_url: None,
                };
                ContentImage::insert(content_image::ActiveModel::from(pending_image))
                    .exec(tx)
                    .await
                    .map_err(|e| {
                        Error::Database(format!("Error inserting pending image: {}", e))
                    })?;
            }
            Ok::<(), Error>(())
        })
    })
    .await
    .map_err(|e| Error::Database(format!("Error saving pending article: {}", e)))?;
    Ok(())
}

pub async fn save_article(db: &DatabaseConnection, article: Article) -> Result<(), Error> {
    let existing = Content::find_by_id(article.id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking existing article: {}", e)))?;
    let slug = if let Some(existing) = &existing {
        existing.slug.clone()
    } else {
        next_slug_for_title(db, &article.title)
            .await
            .unwrap_or(article.id.to_string())
    };
    let now = chrono::Utc::now().naive_local();
    let recovered_from_dead_link = existing
        .as_ref()
        .map(|existing| existing.recovered_from_dead_link)
        .unwrap_or(false);
    let first_image_id = article
        .images
        .first()
        .ok_or(Error::ImageGeneration("No images generated".into()))?
        .id
        .clone();
    let c = build_saved_content_model(
        SavedContentInput {
            id: article.id.to_string(),
            slug,
            markdown: article.markdown.clone(),
            prompt_version: article.prompt_version,
            start_time: article.start_time,
            model: article.model,
            description: article.description,
            image_id: None,
            title: article.title,
            instructions: article.instructions,
            author_email: article.author_email.clone(),
            recovered_from_dead_link,
        },
        now,
    );

    let mut c = content::ActiveModel::from(c);
    if existing.is_some() {
        c = c.reset_all();
    }
    db.transaction(|tx| {
        Box::pin(async move {
            if Content::find_by_id(article.id.clone())
                .one(tx)
                .await
                .map_err(|e| Error::Database(format!("Error finding content: {}", e)))?
                .is_some()
            {
                Content::update(c.clone())
                    .exec(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error updating content: {}", e)))?;
            } else {
                Content::insert(c.clone())
                    .exec(tx)
                    .await
                    .map_err(|e| Error::Database(format!("Error inserting content: {}", e)))?;
            }
            for img in article.images {
                save_generated_image(
                    article.id.clone(),
                    img.img.prompt.clone(),
                    img.img.caption.clone(),
                    img.parameters,
                    img.id.clone(),
                    tx,
                    img.data,
                )
                .await?;
            }
            c.image_id = ActiveValue::set(Some(first_image_id));
            Content::update(c)
                .filter(content::Column::Id.eq(article.id))
                .exec(tx)
                .await
                .map_err(|e| Error::Database(format!("Error updating content: {}", e)))?;
            Ok::<(), Error>(())
        })
    })
    .await
    .map_err(|e| Error::Database(format!("Error creating article: {}", e)))?;
    Ok(())
}
