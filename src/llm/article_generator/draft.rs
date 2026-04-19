use crate::error::Error;
use crate::image_generator::ImageToCreate;
use crate::llm::Llm;

use super::image_briefs::generate_image_briefs;
use super::prompt_builder::{
    build_article_messages, build_placeholder_messages, build_research_article_messages,
};
use super::validation::{
    ensure_image_briefs_present, ensure_minimum_paragraph_count, parse_titled_markdown,
    split_paragraphs, ParsedArticleDraft,
};

pub struct ArticleData {
    pub title: String,
    pub paragraphs: Vec<String>,
    pub images: Vec<ImageToCreate>,
}

pub async fn generate_article_parts(
    llm: &Llm,
    _examples: Vec<(String, String)>,
    instructions: &str,
    model: &str,
) -> Result<ArticleData, Error> {
    let article = request_article_draft(llm, instructions, model).await?;
    let article = parse_titled_markdown(&article)?;
    let paragraphs = split_paragraphs(&article.body);
    ensure_minimum_paragraph_count(&paragraphs)?;
    let images = generate_image_briefs(llm, &article.body, model).await?;
    ensure_image_briefs_present(&images)?;

    Ok(ArticleData {
        title: article.title,
        paragraphs,
        images,
    })
}

pub async fn generate_placeholder_article_draft(
    llm: &Llm,
    instructions: &str,
    model: &str,
    examples: Option<Vec<(String, String)>>,
) -> Result<ParsedArticleDraft, Error> {
    let article = llm
        .request_chat(build_placeholder_messages(examples, instructions), model)
        .await?
        .trim()
        .to_string();

    parse_titled_markdown(&article)
}

pub async fn request_article_draft(
    llm: &Llm,
    instructions: &str,
    model: &str,
) -> Result<String, Error> {
    llm.request_chat(build_article_messages(instructions), model)
        .await
        .map(|article| article.trim().to_string())
}

pub async fn request_researched_article_draft(
    llm: &Llm,
    instructions: &str,
    model: &str,
) -> Result<String, Error> {
    llm.request_chat(build_research_article_messages(instructions), model)
        .await
        .map(|article| article.trim().to_string())
}
