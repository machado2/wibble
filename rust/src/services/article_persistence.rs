use sea_orm::prelude::*;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, EntityTrait, QueryFilter,
    QuerySelect,
};
use slugify::slugify;
use uuid::Uuid;

use crate::entities::{content, content_image, prelude::*};
use crate::error::Error;

pub struct SaveContentRequest {
    pub id: String,
    pub markdown: String,
    pub prompt_version: i32,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub image_id: Option<String>,
    pub title: String,
    pub instructions: String,
    pub author_email: Option<String>,
}

pub struct PreparedContentUpsert {
    content: content::ActiveModel,
    has_existing: bool,
}

struct SavedContentModelInput {
    request: SaveContentRequest,
    slug: String,
    recovered_from_dead_link: bool,
}

pub async fn next_slug_for_title(db: &DatabaseConnection, title: &str) -> Result<String, Error> {
    let base_slug = slugify!(title);
    if base_slug.trim().is_empty() {
        return Ok(Uuid::new_v4().to_string());
    }

    let slug_prefix = format!("{}-", base_slug);
    let existing_slugs = Content::find()
        .filter(
            Condition::any()
                .add(content::Column::Slug.eq(base_slug.clone()))
                .add(content::Column::Slug.starts_with(&slug_prefix)),
        )
        .select_only()
        .column(content::Column::Slug)
        .into_tuple::<String>()
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking for slug: {}", e)))?;

    Ok(next_available_slug(&base_slug, &existing_slugs))
}

pub async fn prepare_content_upsert(
    db: &DatabaseConnection,
    request: SaveContentRequest,
) -> Result<PreparedContentUpsert, Error> {
    let existing = Content::find_by_id(request.id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking existing article: {}", e)))?;
    let slug = if let Some(existing) = &existing {
        existing.slug.clone()
    } else {
        next_slug_for_title(db, &request.title)
            .await
            .unwrap_or_else(|_| request.id.clone())
    };
    let now = chrono::Utc::now().naive_local();
    let recovered_from_dead_link = existing
        .as_ref()
        .map(|existing| existing.recovered_from_dead_link)
        .unwrap_or(false);
    let has_existing = existing.is_some();
    let mut content = content::ActiveModel::from(build_saved_content_model(
        SavedContentModelInput {
            request,
            slug,
            recovered_from_dead_link,
        },
        now,
    ));
    if has_existing {
        content = content.reset_all();
    }

    Ok(PreparedContentUpsert {
        content,
        has_existing,
    })
}

pub async fn upsert_prepared_content(
    tx: &DatabaseTransaction,
    prepared: PreparedContentUpsert,
) -> Result<(), Error> {
    if prepared.has_existing {
        Content::update(prepared.content)
            .exec(tx)
            .await
            .map_err(|e| Error::Database(format!("Error updating content: {}", e)))?;
    } else {
        Content::insert(prepared.content)
            .exec(tx)
            .await
            .map_err(|e| Error::Database(format!("Error inserting content: {}", e)))?;
    }
    Ok(())
}

pub async fn replace_content_images(
    tx: &DatabaseTransaction,
    content_id: &str,
) -> Result<(), Error> {
    ContentImage::delete_many()
        .filter(content_image::Column::ContentId.eq(content_id))
        .exec(tx)
        .await
        .map_err(|e| Error::Database(format!("Error deleting content images: {}", e)))?;
    Ok(())
}

fn build_saved_content_model(input: SavedContentModelInput, now: DateTime) -> content::Model {
    let published = input.request.author_email.is_none();
    content::Model {
        id: input.request.id,
        slug: input.slug,
        content: Some(input.request.markdown.clone()),
        created_at: now,
        generating: false,
        generation_started_at: Some(input.request.start_time),
        generation_finished_at: Some(now),
        flagged: false,
        model: input.request.model,
        prompt_version: input.request.prompt_version,
        fail_count: 0,
        description: input.request.description,
        image_id: input.request.image_id,
        title: input.request.title,
        user_input: input.request.instructions,
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: Some(input.request.markdown),
        converted: false,
        longview_count: 0,
        impression_count: 0,
        click_count: 0,
        author_email: input.request.author_email,
        published,
        recovered_from_dead_link: input.recovered_from_dead_link,
    }
}

fn next_available_slug(base_slug: &str, existing_slugs: &[String]) -> String {
    if base_slug.is_empty() {
        return Uuid::new_v4().to_string();
    }
    if existing_slugs.is_empty() {
        return base_slug.to_string();
    }

    let mut next_suffix = 2u32;
    let prefix = format!("{}-", base_slug);
    for slug in existing_slugs {
        if slug == base_slug {
            continue;
        }
        if let Some(suffix) = slug
            .strip_prefix(&prefix)
            .and_then(|value| value.parse::<u32>().ok())
        {
            next_suffix = next_suffix.max(suffix.saturating_add(1));
        }
    }

    format!("{}-{}", base_slug, next_suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_time() -> DateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 4, 18)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
    }

    #[test]
    fn next_available_slug_returns_base_when_unused() {
        assert_eq!(next_available_slug("article-title", &[]), "article-title");
    }

    #[test]
    fn next_available_slug_skips_to_highest_numeric_suffix() {
        let existing = vec![
            "article-title".to_string(),
            "article-title-2".to_string(),
            "article-title-4".to_string(),
        ];

        assert_eq!(
            next_available_slug("article-title", &existing),
            "article-title-5"
        );
    }

    #[test]
    fn build_saved_content_model_publishes_articles_without_author_email() {
        let model = build_saved_content_model(
            SavedContentModelInput {
                request: SaveContentRequest {
                    id: "article-id".to_string(),
                    markdown: "# Title\n\nBody".to_string(),
                    prompt_version: 11,
                    start_time: sample_time(),
                    model: "test-model".to_string(),
                    description: "desc".to_string(),
                    image_id: Some("image-id".to_string()),
                    title: "Title".to_string(),
                    instructions: "prompt".to_string(),
                    author_email: None,
                },
                slug: "article-slug".to_string(),
                recovered_from_dead_link: false,
            },
            sample_time(),
        );

        assert!(model.published);
    }

    #[test]
    fn build_saved_content_model_creates_draft_for_authenticated_authors() {
        let model = build_saved_content_model(
            SavedContentModelInput {
                request: SaveContentRequest {
                    id: "article-id".to_string(),
                    markdown: "# Title\n\nBody".to_string(),
                    prompt_version: 12,
                    start_time: sample_time(),
                    model: "test-model".to_string(),
                    description: "desc".to_string(),
                    image_id: None,
                    title: "Title".to_string(),
                    instructions: "prompt".to_string(),
                    author_email: Some("author@example.com".to_string()),
                },
                slug: "article-slug".to_string(),
                recovered_from_dead_link: false,
            },
            sample_time(),
        );

        assert!(!model.published);
    }

    #[test]
    fn build_saved_content_model_preserves_dead_link_recovery_flag() {
        let model = build_saved_content_model(
            SavedContentModelInput {
                request: SaveContentRequest {
                    id: "article-id".to_string(),
                    markdown: "# Title\n\nBody".to_string(),
                    prompt_version: 13,
                    start_time: sample_time(),
                    model: "test-model".to_string(),
                    description: "desc".to_string(),
                    image_id: None,
                    title: "Title".to_string(),
                    instructions: "prompt".to_string(),
                    author_email: None,
                },
                slug: "article-slug".to_string(),
                recovered_from_dead_link: true,
            },
            sample_time(),
        );

        assert!(model.recovered_from_dead_link);
    }

    #[test]
    fn build_saved_content_model_preserves_prompt_version() {
        let model = build_saved_content_model(
            SavedContentModelInput {
                request: SaveContentRequest {
                    id: "article-id".to_string(),
                    markdown: "# Title\n\nBody".to_string(),
                    prompt_version: 42,
                    start_time: sample_time(),
                    model: "test-model".to_string(),
                    description: "desc".to_string(),
                    image_id: None,
                    title: "Title".to_string(),
                    instructions: "prompt".to_string(),
                    author_email: None,
                },
                slug: "article-slug".to_string(),
                recovered_from_dead_link: false,
            },
            sample_time(),
        );

        assert_eq!(model.prompt_version, 42);
    }
}
