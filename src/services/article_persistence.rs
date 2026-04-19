use sea_orm::prelude::*;
use sea_orm::{ColumnTrait, Condition, QueryFilter, QuerySelect};
use slugify::slugify;
use uuid::Uuid;

use crate::entities::{content, prelude::*};
use crate::error::Error;

pub struct SavedContentInput {
    pub id: String,
    pub slug: String,
    pub markdown: String,
    pub start_time: DateTime,
    pub model: String,
    pub description: String,
    pub image_id: Option<String>,
    pub title: String,
    pub instructions: String,
    pub author_email: Option<String>,
    pub recovered_from_dead_link: bool,
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

pub fn build_saved_content_model(input: SavedContentInput, now: DateTime) -> content::Model {
    content::Model {
        id: input.id,
        slug: input.slug,
        content: Some(input.markdown.clone()),
        created_at: now,
        generating: false,
        generation_started_at: Some(input.start_time),
        generation_finished_at: Some(now),
        flagged: false,
        model: input.model,
        prompt_version: 0,
        fail_count: 0,
        description: input.description,
        image_id: input.image_id,
        title: input.title,
        user_input: input.instructions,
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: Some(input.markdown),
        converted: false,
        longview_count: 0,
        impression_count: 0,
        click_count: 0,
        author_email: input.author_email,
        published: true,
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
            SavedContentInput {
                id: "article-id".to_string(),
                slug: "article-slug".to_string(),
                markdown: "# Title\n\nBody".to_string(),
                start_time: sample_time(),
                model: "test-model".to_string(),
                description: "desc".to_string(),
                image_id: Some("image-id".to_string()),
                title: "Title".to_string(),
                instructions: "prompt".to_string(),
                author_email: None,
                recovered_from_dead_link: false,
            },
            sample_time(),
        );

        assert!(model.published);
    }

    #[test]
    fn build_saved_content_model_keeps_articles_published_with_author_email() {
        let model = build_saved_content_model(
            SavedContentInput {
                id: "article-id".to_string(),
                slug: "article-slug".to_string(),
                markdown: "# Title\n\nBody".to_string(),
                start_time: sample_time(),
                model: "test-model".to_string(),
                description: "desc".to_string(),
                image_id: None,
                title: "Title".to_string(),
                instructions: "prompt".to_string(),
                author_email: Some("author@example.com".to_string()),
                recovered_from_dead_link: false,
            },
            sample_time(),
        );

        assert!(model.published);
    }

    #[test]
    fn build_saved_content_model_preserves_dead_link_recovery_flag() {
        let model = build_saved_content_model(
            SavedContentInput {
                id: "article-id".to_string(),
                slug: "article-slug".to_string(),
                markdown: "# Title\n\nBody".to_string(),
                start_time: sample_time(),
                model: "test-model".to_string(),
                description: "desc".to_string(),
                image_id: None,
                title: "Title".to_string(),
                instructions: "prompt".to_string(),
                author_email: None,
                recovered_from_dead_link: true,
            },
            sample_time(),
        );

        assert!(model.recovered_from_dead_link);
    }
}
