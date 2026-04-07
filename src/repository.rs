use rand::prelude::*;
use sea_orm::prelude::*;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, QueryFilter, QuerySelect, TransactionTrait,
};
use serde_json::Value;
use slugify::slugify;
use std::path::PathBuf;
use std::{env, fs};
use uuid::Uuid;

use crate::entities::prelude::*;
use crate::entities::{content, content_image, examples};
use crate::error::Error;
use crate::image_generator::{ImageGenerated, ImageToCreate};
use crate::image_status::{IMAGE_STATUS_COMPLETED, IMAGE_STATUS_PENDING};
use crate::s3;

pub async fn get_examples(db: &DatabaseConnection) -> Result<Vec<(String, String)>, Error> {
    let k = || async {
        // Step 1: Get the maximum new_id
        let max_id = Examples::find()
            .select_only()
            .column_as(examples::Column::NewId.max(), "max_new_id")
            .into_tuple::<Option<i32>>()
            .one(db)
            .await?
            .flatten();

        if let Some(max_id) = max_id {
            // Step 2: Generate random new_id values
            let random_ids: Vec<i32> = (0..3)
                .map(|_| rand::rng().random_range(1..=max_id))
                .collect();
            // Step 3: Fetch rows based on random new_id values
            let examples = Examples::find()
                .filter(examples::Column::NewId.is_in(random_ids.clone()))
                .all(db)
                .await?;

            // Process the results
            Ok(examples
                .into_iter()
                .filter_map(|example| {
                    let first_line = example.content.as_deref()?.lines().next().unwrap_or("");
                    let content = example.content.clone().unwrap_or_default();

                    let user_input = if example.user_input.starts_with('{') {
                        let json: Value = serde_json::from_str(&example.user_input).ok()?;
                        json["suggestion"].as_str().map(String::from)
                    } else {
                        None
                    }
                    .unwrap_or(example.user_input);

                    if !first_line.starts_with('#') {
                        let titled_content = format!("# {}\n{}", example.title, content);
                        Some((user_input, titled_content))
                    } else {
                        Some((user_input, content))
                    }
                })
                .collect())
        } else {
            // Handle the case where the table is empty
            Ok(Vec::new())
        }
    };
    k().await.map_err(|e: DbErr| Error::Database(e.to_string()))
}

async fn get_slug_for(db: &DatabaseConnection, title: &str) -> Result<String, Error> {
    let slug = slugify!(title);
    if Content::find()
        .filter(content::Column::Slug.contains(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking for slug: {}", e)))?
        .is_none()
    {
        Ok(slug)
    } else {
        Ok(Uuid::new_v4().to_string())
    }
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

async fn save_image(
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

pub struct Article {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub instructions: String,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub images: Vec<ImageGenerated>,
}

pub struct PendingArticle {
    pub id: String,
    pub title: String,
    pub markdown: String,
    pub instructions: String,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub images: Vec<ImageToCreate>,
    pub image_generator: String,
}

pub async fn save_pending_article(
    db: &DatabaseConnection,
    article: PendingArticle,
) -> Result<(), Error> {
    let PendingArticle {
        id,
        title,
        markdown,
        instructions,
        start_time,
        model,
        description,
        images,
        image_generator,
    } = article;
    let existing = Content::find_by_id(id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking existing article: {}", e)))?;
    let has_existing = existing.is_some();
    let slug = if let Some(existing) = &existing {
        existing.slug.clone()
    } else {
        get_slug_for(db, &title).await.unwrap_or(id.to_string())
    };
    let now = chrono::Utc::now().naive_local();
    let first_image_id = images
        .first()
        .ok_or(Error::ImageGeneration("No images planned".into()))?
        .id
        .clone();
    let c = content::Model {
        id: id.to_string(),
        slug,
        content: Some(markdown.clone()),
        created_at: now,
        generating: false,
        generation_started_at: Some(start_time),
        generation_finished_at: Some(now),
        flagged: false,
        model,
        prompt_version: 0,
        fail_count: 0,
        description,
        image_id: Some(first_image_id),
        title,
        user_input: instructions,
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: Some(markdown.clone()),
        converted: false,
        longview_count: 0,
        impression_count: 0,
        click_count: 0,
    };

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
        get_slug_for(db, &article.title)
            .await
            .unwrap_or(article.id.to_string())
    };
    let now = chrono::Utc::now().naive_local();
    let first_image_id = article
        .images
        .first()
        .ok_or(Error::ImageGeneration("No images generated".into()))?
        .id
        .clone();
    let c = content::Model {
        id: article.id.to_string(),
        slug,
        content: Some(article.markdown.clone()),
        created_at: now,
        generating: false,
        generation_started_at: Some(article.start_time),
        generation_finished_at: Some(now),
        flagged: false,
        model: article.model,
        prompt_version: 0,
        fail_count: 0,
        description: article.description,
        image_id: None,
        title: article.title,
        user_input: article.instructions,
        // view_count removed
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: Some(article.markdown.clone()),
        converted: false,
        // lemmy-related fields removed
        longview_count: 0,
        // umami_view_count removed
        impression_count: 0,
        click_count: 0,
    };

    let mut c = content::ActiveModel::from(c);
    // When updating an existing placeholder row (dead-link recovery), convert
    // unchanged fields into explicit SETs so markdown/content/generating are persisted.
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
                save_image(
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
