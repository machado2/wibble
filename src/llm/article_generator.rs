use regex::Regex;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::error::Error;
use crate::image_generator::{generate_images, ImageGenerated, ImageToCreate};
use crate::llm::{Llm, Message};
use crate::repository::{get_examples, save_article, Article};

static SYSTEM_MESSAGE_ARTICLE: &str = include_str!("../../prompts/system_article.txt");
static SYSTEM_WITH_PLACEHOLDERS: &str = include_str!("../../prompts/system_with_placeholders.txt");
static SYSTEM_MESSAGE_ILLUSTRATOR: &str = include_str!("../../prompts/illustrator.txt");

fn format_messages(
    system_message: &str,
    examples: Option<Vec<(String, String)>>,
    instructions: &str,
) -> Vec<Message> {
    let mut messages = Vec::<Message>::new();
    messages.push(Message::System(system_message.to_string()));
    if let Some(examples) = examples {
        for (prompt, article) in examples {
            messages.push(Message::User(prompt.to_string()));
            messages.push(Message::Assistant(article));
        }
    }
    messages.push(Message::User(instructions.to_string()));
    messages
}

fn split_title(markdown: &str) -> Option<(String, String)> {
    let mut lines = markdown.lines();
    while let Some(line) = lines.next() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let title = line
            .trim_start_matches('#')
            .trim_start_matches("Title")
            .trim_start_matches(':')
            .trim();
        let remaining = lines.collect::<Vec<&str>>().join("\n");
        return Some((title.to_string(), remaining.to_string()));
    }
    None
}

async fn create_image_prompts(
    llm: &Llm,
    article: &str,
    model: &str,
) -> Result<Vec<ImageToCreate>, Error> {
    let messages = format_messages(SYSTEM_MESSAGE_ILLUSTRATOR, None, article);
    let images = llm.request_chat(messages, model).await?;
    let images = images
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(2, ';');
            let caption = parts.next().unwrap_or("").to_string();
            let prompt = parts.next().unwrap_or("").to_string();
            if caption.is_empty() || prompt.is_empty() {
                return None;
            }
            Some(ImageToCreate {
                id: Uuid::new_v4().to_string(),
                caption,
                prompt,
            })
        })
        .collect();
    Ok(images)
}

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
    let messages = format_messages(SYSTEM_MESSAGE_ARTICLE, None, instructions);
    let article = llm.request_chat(messages, model).await?.trim().to_string();
    let (title, article) =
        split_title(&article).ok_or(Error::Llm("No title found in article".to_string()))?;
    let paragraphs: Vec<_> = article.split("\n\n").map(|p| p.to_string()).collect();
    if paragraphs.len() < 4 {
        return Err(Error::Llm("Article has less than 4 paragraphs".to_string()));
    }
    let images = create_image_prompts(llm, &article, model).await?;
    if images.is_empty() {
        return Err(Error::Llm("Failed to generate image prompts".to_string()));
    }
    Ok(ArticleData {
        title,
        paragraphs,
        images,
    })
}

fn img_to_markdown(img: &ImageGenerated) -> String {
    format!(
        "![{}](/image/{} \"{}\")",
        img.img.caption, img.id, img.img.caption
    )
}

pub async fn create_article_using_placeholders(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
    use_examples: bool,
) -> Result<(), Error> {
    let llm = &state.llm;

    let examples = if use_examples {
        Some(get_examples(&state.db).await?)
    } else {
        None
    };

    let messages = format_messages(SYSTEM_WITH_PLACEHOLDERS, examples, &instructions);

    let article = llm.request_chat(messages, model).await?.trim().to_string();
    let (title, article) =
        split_title(&article).ok_or(Error::Llm("No title found in article".to_string()))?;

    let mut markdown = article.to_string();
    let mut images = Vec::new();
    for cap in Regex::new(r#"<GeneratedImage prompt="([^"]+)" alt="([^"]+)" />"#)
        .map_err(|e| Error::Llm(format!("Error creating regex: {}", e)))?
        .captures_iter(&article)
    {
        let prompt = cap[1].to_string();
        let alt = cap[2].to_string();
        let id = Uuid::new_v4().to_string();
        let markdown_img = format!("![{}](/image/{} \"{}\")", prompt, id, alt);
        markdown = markdown.replacen(&cap[0], &markdown_img, 1);
        images.push(ImageToCreate {
            id,
            prompt,
            caption: alt,
        });
    }

    let images = generate_images(state, images).await?;

    let description = markdown.split("\n\n").next().unwrap_or("").to_string();
    save_article(
        &state.db,
        Article {
            id,
            title,
            markdown,
            instructions,
            start_time: chrono::Utc::now().naive_local(),
            model: model.to_string(),
            description,
            images,
        },
    )
    .await?;

    Ok(())
}

pub async fn create_article_attempt(
    state: &AppState,
    id: String,
    instructions: String,
    model: &str,
) -> Result<(), Error> {
    let db = &state.db;
    let start_time = chrono::Utc::now().naive_local();
    let examples = get_examples(&state.db).await?;
    let article = generate_article_parts(&state.llm, examples, &instructions, model).await?;
    let paragraphs = article.paragraphs;
    let images = generate_images(state, article.images).await?;

    let mut remaining_images = images.iter();

    let mut markdown = Vec::<String>::new();
    let mut par_per_img = paragraphs.len() / images.len();
    if par_per_img == 0 {
        par_per_img = 1;
    }

    let mut current_p = 0;
    for p in paragraphs {
        markdown.push(p);
        current_p += 1;
        if current_p % par_per_img == 0 {
            let img = remaining_images.next();
            if let Some(img) = img {
                markdown.push(img_to_markdown(img));
            }
        }
    }

    for img in remaining_images {
        markdown.push(img_to_markdown(img));
    }

    let description = markdown[0].clone();
    let markdown = markdown.join("\n\n");

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
        },
    )
    .await?;
    Ok(())
}
