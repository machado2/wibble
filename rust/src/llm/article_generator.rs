mod draft;
mod image_briefs;
mod planning;
mod prompt_builder;
mod research;
mod runtime;
mod validation;

use crate::app_state::AppState;
use crate::error::Error;
use crate::image_generator::generate_images;
use crate::image_jobs::enqueue_pending_images;
use crate::llm::prompt_registry::{
    article_generation_prompt, placeholder_generation_prompt, research_article_generation_prompt,
};
use crate::repositories::{
    articles::{save_article, save_pending_article, Article, PendingArticle},
    examples::get_examples,
};
use crate::services::article_jobs::ArticleJobService;

pub use draft::{generate_article_parts, ArticleData};
use draft::{
    generate_placeholder_article_draft, request_article_draft, request_researched_article_draft,
};
use image_briefs::{generate_image_briefs, replace_placeholder_tags_with_markdown};
use planning::{compose_article_markdown, leading_paragraph};
use research::gather_research_packet;
pub use research::{prompt_requires_research, resolve_research_mode, ResearchModeSource};
use runtime::{BoundedGenerationRuntime, GenerationTool};
use validation::{
    ensure_generated_images_present, ensure_image_briefs_present,
    ensure_placeholder_images_present, parse_titled_markdown, validate_researched_article_output,
};
pub use validation::{ensure_minimum_paragraph_count, split_paragraphs, validate_article_output};

pub async fn create_article_using_placeholders(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
    use_examples: bool,
    author_email: Option<String>,
) -> Result<(), Error> {
    let llm = &state.llm;
    let mut runtime =
        BoundedGenerationRuntime::new(state.clone(), id.clone(), &instructions).await?;
    runtime
        .begin_tool(GenerationTool::ArticlePlanning, false)
        .await?;
    let examples = if use_examples {
        Some(get_examples(&state.db).await?)
    } else {
        None
    };

    runtime
        .begin_tool(GenerationTool::DraftWriter, true)
        .await?;
    let article = generate_placeholder_article_draft(llm, &instructions, model, examples).await?;
    runtime
        .begin_tool(GenerationTool::ImageBriefPlanner, false)
        .await?;
    let placeholder_images = replace_placeholder_tags_with_markdown(&article.body)?;
    ensure_placeholder_images_present(&placeholder_images.images)?;
    runtime
        .begin_tool(GenerationTool::PolicyCheck, false)
        .await?;
    validate_article_output(
        &article.title,
        &placeholder_images.markdown,
        placeholder_images.images.len(),
    )?;
    runtime.mark_ready_for_review().await?;
    runtime.ensure_not_cancelled().await?;

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
            prompt_version: placeholder_generation_prompt().version,
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
    let mut runtime =
        BoundedGenerationRuntime::new(state.clone(), id.clone(), &instructions).await?;
    runtime
        .begin_tool(GenerationTool::ArticlePlanning, false)
        .await?;
    runtime
        .begin_tool(GenerationTool::DraftWriter, true)
        .await?;
    let article = request_article_draft(&state.llm, &instructions, model).await?;
    let article = parse_titled_markdown(&article)?;
    let paragraphs = split_paragraphs(&article.body);
    ensure_minimum_paragraph_count(&paragraphs)?;
    runtime
        .begin_tool(GenerationTool::ImageBriefPlanner, true)
        .await?;
    let image_briefs = generate_image_briefs(&state.llm, &article.body, model).await?;
    ensure_image_briefs_present(&image_briefs)?;
    let images = generate_images(state, image_briefs).await?;
    ensure_generated_images_present(&images)?;

    let markdown = compose_article_markdown(paragraphs, &images);
    runtime
        .begin_tool(GenerationTool::PolicyCheck, false)
        .await?;
    validate_article_output(&article.title, &markdown, images.len())?;
    runtime.mark_ready_for_review().await?;
    runtime.ensure_not_cancelled().await?;
    let description = leading_paragraph(&markdown);

    save_article(
        db,
        Article {
            id,
            title: article.title,
            markdown,
            prompt_version: article_generation_prompt().version,
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

pub async fn create_researched_article_attempt(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
    author_email: Option<String>,
    mode_source: ResearchModeSource,
) -> Result<(), Error> {
    let db = &state.db;
    let start_time = chrono::Utc::now().naive_local();
    let mut runtime =
        BoundedGenerationRuntime::new_research(state.clone(), id.clone(), &instructions).await?;
    runtime
        .begin_tool(GenerationTool::ArticlePlanning, false)
        .await?;
    let research = gather_research_packet(state, &mut runtime, &instructions, mode_source).await?;
    ArticleJobService::new(state.clone())
        .merge_preview_payload(&id, research.preview_payload())
        .await?;
    runtime
        .begin_tool(GenerationTool::DraftWriter, true)
        .await?;
    let researched_prompt = research.prompt_context(&instructions);
    let article = request_researched_article_draft(&state.llm, &researched_prompt, model).await?;
    let article = parse_titled_markdown(&article)?;
    let paragraphs = split_paragraphs(&article.body);
    ensure_minimum_paragraph_count(&paragraphs)?;
    runtime
        .begin_tool(GenerationTool::ImageBriefPlanner, true)
        .await?;
    let image_briefs = generate_image_briefs(&state.llm, &article.body, model).await?;
    ensure_image_briefs_present(&image_briefs)?;
    let images = generate_images(state, image_briefs).await?;
    ensure_generated_images_present(&images)?;

    let markdown = compose_article_markdown(paragraphs, &images);
    runtime
        .begin_tool(GenerationTool::PolicyCheck, false)
        .await?;
    validate_researched_article_output(&article.title, &markdown, images.len(), &research.sources)?;
    runtime.mark_ready_for_review().await?;
    runtime.ensure_not_cancelled().await?;
    let description = leading_paragraph(&markdown);

    save_article(
        db,
        Article {
            id,
            title: article.title,
            markdown,
            prompt_version: research_article_generation_prompt().version,
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
