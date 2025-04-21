use rand::prelude::*;
use sea_orm::prelude::*;
use sea_orm::{ActiveValue, QuerySelect, TransactionTrait};
use serde_json::Value;
use slugify::slugify;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

use crate::entities::prelude::*;
use crate::entities::{content, content_image, examples};
use crate::error::Error;
use crate::image_generator::ImageGenerated;

pub async fn get_examples(db: &DatabaseConnection) -> Result<Vec<(String, String)>, Error> {
    let k = || async {
        // Step 1: Get the maximum new_id
        let max_id = Examples::find()
            .select_only()
            .column_as(examples::Column::NewId.max(), "max_new_id")
            .into_tuple::<Option<i64>>()
            .one(db)
            .await?
            .flatten();

        if let Some(max_id) = max_id {
            // Step 2: Generate random new_id values
            let random_ids: Vec<i64> = (0..3).map(|_| rand::rng().random_range(1..=max_id)).collect();
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

async fn save_image(
    article_id: String,
    prompt: String,
    alt_text: String,
    parameters: String,
    id: String,
    db: &impl ConnectionTrait,
    img: Vec<u8>,
) -> Result<(), Error> {
    let content_image = content_image::Model {
        id: id.clone(),
        content_id: article_id,
        prompt: prompt.clone(),
        alt_text,
        created_at: chrono::Utc::now().naive_local(),
        model: None,
        fail_count: 0,
        flagged: false,
        generator: None,
        parameters: Some(parameters),
        prompt_hash: None,
        regenerate: false,
        seed: None,
        view_count: 0,
    };
    ContentImage::insert(content_image::ActiveModel::from(content_image))
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting content_image: {}", e)))?;
    
    let image_path = PathBuf::from("static/images").join(format!("{}.jpg", id));
    fs::create_dir_all(image_path.parent().unwrap())
        .map_err(|e| Error::Image(image::ImageError::IoError(e)))?;
    fs::write(&image_path, img)
        .map_err(|e| Error::Image(image::ImageError::IoError(e)))?;
    
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

pub async fn save_article(db: &DatabaseConnection, article: Article) -> Result<(), Error> {
    let slug = get_slug_for(db, &article.title)
        .await
        .unwrap_or(article.id.to_string());
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
        view_count: 0,
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: Some(article.markdown.clone()),
        converted: false,
        lemmy_id: None,
        last_lemmy_post_attempt: None,
        longview_count: 0,
        umami_view_count: 0,
    };

    let mut c = content::ActiveModel::from(c);
    db.transaction(|tx| {
        Box::pin(async move {
            Content::insert(c.clone())
                .exec(tx)
                .await
                .map_err(|e| Error::Database(format!("Error inserting content: {}", e)))?;
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
