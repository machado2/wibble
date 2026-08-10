use sea_orm::prelude::*;
use sea_orm::{DatabaseConnection, EntityTrait, TransactionTrait};

use crate::entities::{content_image, prelude::*};
use crate::error::Error;
use crate::image_generator::{ImageGenerated, ImageToCreate};
use crate::image_status::IMAGE_STATUS_PENDING;
use crate::services::article_persistence::{
    prepare_content_upsert, replace_content_images, upsert_prepared_content, SaveContentRequest,
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
    let now = chrono::Utc::now().naive_local();
    let first_image_id = images
        .first()
        .ok_or(Error::ImageGeneration("No images planned".into()))?
        .id
        .clone();
    let prepared = prepare_content_upsert(
        db,
        SaveContentRequest {
            id: id.clone(),
            markdown: markdown.clone(),
            prompt_version,
            start_time,
            model,
            description,
            image_id: Some(first_image_id),
            title,
            instructions,
            author_email,
        },
    )
    .await?;
    db.transaction(|tx| {
        Box::pin(async move {
            upsert_prepared_content(tx, prepared).await?;
            replace_content_images(tx, &id).await?;

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
    let Article {
        id,
        title,
        markdown,
        prompt_version,
        instructions,
        start_time,
        model,
        description,
        images,
        author_email,
    } = article;
    let first_image_id = images
        .first()
        .ok_or(Error::ImageGeneration("No images generated".into()))?
        .id
        .clone();
    let prepared = prepare_content_upsert(
        db,
        SaveContentRequest {
            id: id.clone(),
            markdown,
            prompt_version,
            start_time,
            model,
            description,
            image_id: Some(first_image_id),
            title,
            instructions,
            author_email,
        },
    )
    .await?;
    db.transaction(|tx| {
        Box::pin(async move {
            upsert_prepared_content(tx, prepared).await?;
            replace_content_images(tx, &id).await?;

            for img in images {
                save_generated_image(
                    id.clone(),
                    img.img.prompt.clone(),
                    img.img.caption.clone(),
                    img.parameters,
                    img.id.clone(),
                    tx,
                    img.data,
                )
                .await?;
            }
            Ok::<(), Error>(())
        })
    })
    .await
    .map_err(|e| Error::Database(format!("Error creating article: {}", e)))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sea_orm::{sea_query::Expr, ColumnTrait, EntityTrait, QueryFilter};

    use crate::entities::{content, content_image};
    use crate::test_support::TestContext;

    use super::*;

    fn generated_image(id: &str, prompt: &str, caption: &str) -> ImageGenerated {
        ImageGenerated {
            id: id.to_string(),
            img: ImageToCreate {
                id: id.to_string(),
                caption: caption.to_string(),
                prompt: prompt.to_string(),
            },
            data: vec![1, 2, 3, 4],
            parameters: "{\"style\":\"test\"}".to_string(),
        }
    }

    #[tokio::test]
    async fn save_pending_article_reuses_slug_and_replaces_existing_images() {
        let ctx = TestContext::new().await;
        save_pending_article(
            &ctx.state.db,
            PendingArticle {
                id: "article-1".to_string(),
                title: "Original Title".to_string(),
                markdown: "# Draft".to_string(),
                prompt_version: 1,
                instructions: "prompt".to_string(),
                start_time: chrono::Utc::now().naive_local(),
                model: "test-model".to_string(),
                description: "desc".to_string(),
                images: vec![ImageToCreate {
                    id: "pending-image-1".to_string(),
                    caption: "Caption".to_string(),
                    prompt: "Prompt".to_string(),
                }],
                image_generator: "test-generator".to_string(),
                author_email: Some("author@example.com".to_string()),
            },
        )
        .await
        .unwrap();

        save_pending_article(
            &ctx.state.db,
            PendingArticle {
                id: "article-1".to_string(),
                title: "Updated Title".to_string(),
                markdown: "# Revised Draft".to_string(),
                prompt_version: 2,
                instructions: "updated prompt".to_string(),
                start_time: chrono::Utc::now().naive_local(),
                model: "test-model".to_string(),
                description: "updated desc".to_string(),
                images: vec![ImageToCreate {
                    id: "pending-image-2".to_string(),
                    caption: "New Caption".to_string(),
                    prompt: "New Prompt".to_string(),
                }],
                image_generator: "test-generator".to_string(),
                author_email: Some("author@example.com".to_string()),
            },
        )
        .await
        .unwrap();

        let saved = Content::find_by_id("article-1")
            .one(&ctx.state.db)
            .await
            .unwrap()
            .unwrap();
        let images = ContentImage::find()
            .filter(content_image::Column::ContentId.eq("article-1"))
            .all(&ctx.state.db)
            .await
            .unwrap();

        assert_eq!(saved.slug, "original-title");
        assert_eq!(saved.image_id.as_deref(), Some("pending-image-2"));
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id, "pending-image-2");
    }

    #[tokio::test]
    async fn save_article_replaces_existing_images_and_preserves_existing_flags() {
        let images_dir = std::env::temp_dir().join(format!(
            "wibble-article-images-{}",
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir_all(&images_dir).unwrap();
        let images_dir_value = images_dir.to_string_lossy().to_string();
        let ctx =
            TestContext::new_with_overrides(&[("IMAGES_DIR", images_dir_value.as_str())]).await;

        save_pending_article(
            &ctx.state.db,
            PendingArticle {
                id: "article-2".to_string(),
                title: "Original Title".to_string(),
                markdown: "# Draft".to_string(),
                prompt_version: 1,
                instructions: "prompt".to_string(),
                start_time: chrono::Utc::now().naive_local(),
                model: "test-model".to_string(),
                description: "desc".to_string(),
                images: vec![ImageToCreate {
                    id: "pending-image".to_string(),
                    caption: "Caption".to_string(),
                    prompt: "Prompt".to_string(),
                }],
                image_generator: "test-generator".to_string(),
                author_email: Some("author@example.com".to_string()),
            },
        )
        .await
        .unwrap();

        Content::update_many()
            .col_expr(content::Column::RecoveredFromDeadLink, Expr::value(true))
            .filter(content::Column::Id.eq("article-2"))
            .exec(&ctx.state.db)
            .await
            .unwrap();

        save_article(
            &ctx.state.db,
            Article {
                id: "article-2".to_string(),
                title: "Updated Title".to_string(),
                markdown: "# Published\n\nBody".to_string(),
                prompt_version: 3,
                instructions: "updated prompt".to_string(),
                start_time: chrono::Utc::now().naive_local(),
                model: "test-model".to_string(),
                description: "published desc".to_string(),
                images: vec![generated_image(
                    "generated-image",
                    "New Prompt",
                    "New Caption",
                )],
                author_email: Some("author@example.com".to_string()),
            },
        )
        .await
        .unwrap();

        let saved = Content::find_by_id("article-2")
            .one(&ctx.state.db)
            .await
            .unwrap()
            .unwrap();
        let images = ContentImage::find()
            .filter(content_image::Column::ContentId.eq("article-2"))
            .all(&ctx.state.db)
            .await
            .unwrap();

        assert_eq!(saved.slug, "original-title");
        assert_eq!(saved.image_id.as_deref(), Some("generated-image"));
        assert!(saved.recovered_from_dead_link);
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].id, "generated-image");
        assert!(images_dir.join("generated-image.jpg").exists());
    }
}
