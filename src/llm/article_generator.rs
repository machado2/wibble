mod draft;
mod image_briefs;
mod planning;
mod prompt_builder;
mod validation;

use crate::app_state::AppState;
use crate::error::Error;
use crate::image_generator::generate_images;
use crate::image_jobs::enqueue_pending_images;
use crate::repositories::{
    articles::{save_article, save_pending_article, Article, PendingArticle},
    examples::get_examples,
};

use draft::generate_placeholder_article_draft;
pub use draft::{generate_article_parts, ArticleData};
use image_briefs::replace_placeholder_tags_with_markdown;
use planning::{compose_article_markdown, leading_paragraph};
use validation::{ensure_generated_images_present, ensure_placeholder_images_present};

pub async fn create_article_using_placeholders(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
    use_examples: bool,
    author_email: Option<String>,
) -> Result<(), Error> {
    let llm = &state.llm;
    let examples = if use_examples {
        Some(get_examples(&state.db).await?)
    } else {
        None
    };

    let article = generate_placeholder_article_draft(llm, &instructions, model, examples).await?;
    let placeholder_images = replace_placeholder_tags_with_markdown(&article.body)?;
    ensure_placeholder_images_present(&placeholder_images.images)?;

    let description = leading_paragraph(&placeholder_images.markdown);
    let image_ids = placeholder_images
        .images
        .iter()
        .map(|img| img.id.clone())
        .collect();
    save_pending_article(
        &state.db,
        PendingArticle {
            id,
            title: article.title,
            markdown: placeholder_images.markdown,
            instructions,
            start_time: chrono::Utc::now().naive_local(),
            model: model.to_string(),
            description,
            images: placeholder_images.images,
            image_generator: state.image_generator_name.clone(),
            author_email,
        },
    )
    .await?;
    enqueue_pending_images(state.clone(), image_ids).await;

    Ok(())
}

pub async fn create_article_attempt(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
    author_email: Option<String>,
) -> Result<(), Error> {
    let db = &state.db;
    let start_time = chrono::Utc::now().naive_local();
    let examples = get_examples(&state.db).await?;
    let article = generate_article_parts(&state.llm, examples, &instructions, model).await?;
    let images = generate_images(state, article.images).await?;
    ensure_generated_images_present(&images)?;

    let markdown = compose_article_markdown(article.paragraphs, &images);
    let description = leading_paragraph(&markdown);

    save_article(
        db,
        Article {
            id,
            title: article.title,
            markdown,
            instructions,
            start_time,
            model: model.to_string(),
            description,
            images,
            author_email,
        },
    )
    .await?;
    Ok(())
}
