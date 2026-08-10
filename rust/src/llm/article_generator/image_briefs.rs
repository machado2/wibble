use regex::Regex;
use tracing::{event, Level};
use uuid::Uuid;

use crate::error::Error;
use crate::image_generator::ImageToCreate;
use crate::llm::Llm;

use super::prompt_builder::build_illustrator_messages;

pub struct PlaceholderImages {
    pub markdown: String,
    pub images: Vec<ImageToCreate>,
}

pub async fn generate_image_briefs(
    llm: &Llm,
    article: &str,
    model: &str,
) -> Result<Vec<ImageToCreate>, Error> {
    let response = llm
        .request_chat(build_illustrator_messages(article), model)
        .await?;
    Ok(parse_image_brief_lines(
        &response,
        configured_max_images_per_article(),
    ))
}

pub fn replace_placeholder_tags_with_markdown(article: &str) -> Result<PlaceholderImages, Error> {
    extract_placeholder_images(article, configured_max_images_per_article())
}

fn configured_max_images_per_article() -> usize {
    std::env::var("MAX_IMAGES_PER_ARTICLE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(4)
}

fn parse_image_brief_lines(response: &str, max_images: usize) -> Vec<ImageToCreate> {
    let mut images: Vec<ImageToCreate> = response
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

    truncate_images(&mut images, max_images, "generated image prompts");
    images
}

fn extract_placeholder_images(
    article: &str,
    max_images: usize,
) -> Result<PlaceholderImages, Error> {
    let mut markdown = article.to_string();
    let mut images = Vec::new();
    let placeholder_regex = Regex::new(r#"<GeneratedImage prompt="([^"]+)" alt="([^"]+)" />"#)
        .map_err(|e| Error::Llm(format!("Error creating regex: {}", e)))?;

    for capture in placeholder_regex.captures_iter(article) {
        if images.len() >= max_images {
            event!(
                Level::WARN,
                max_images,
                "Truncating placeholder image tags to MAX_IMAGES_PER_ARTICLE"
            );
            markdown = markdown.replacen(&capture[0], "", 1);
            continue;
        }

        let prompt = capture[1].to_string();
        let alt = capture[2].to_string();
        let id = Uuid::new_v4().to_string();
        let markdown_img = format!("![{}](/image/{} \"{}\")", prompt, id, alt);
        markdown = markdown.replacen(&capture[0], &markdown_img, 1);
        images.push(ImageToCreate {
            id,
            prompt,
            caption: alt,
        });
    }

    Ok(PlaceholderImages { markdown, images })
}

fn truncate_images(images: &mut Vec<ImageToCreate>, max_images: usize, subject: &str) {
    if images.len() > max_images {
        event!(
            Level::WARN,
            max_images,
            original_images = images.len(),
            subject,
            "Truncating article image list to MAX_IMAGES_PER_ARTICLE"
        );
        images.truncate(max_images);
    }
}

#[cfg(test)]
mod tests {
    use super::{extract_placeholder_images, parse_image_brief_lines};

    #[test]
    fn parse_image_brief_lines_skips_malformed_entries() {
        let images = parse_image_brief_lines("Caption;Prompt\nbroken\nSecond;Prompt 2", 10);

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].caption, "Caption");
        assert_eq!(images[1].prompt, "Prompt 2");
    }

    #[test]
    fn parse_image_brief_lines_truncates_to_configured_limit() {
        let images = parse_image_brief_lines("A;1\nB;2\nC;3", 2);

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].caption, "A");
        assert_eq!(images[1].caption, "B");
    }

    #[test]
    fn extract_placeholder_images_rewrites_tags_to_markdown() {
        let placeholder_images = extract_placeholder_images(
            "Paragraph\n\n<GeneratedImage prompt=\"storm\" alt=\"A storm\" />",
            4,
        )
        .unwrap();

        assert_eq!(placeholder_images.images.len(), 1);
        assert!(placeholder_images.markdown.contains("![storm](/image/"));
        assert!(placeholder_images.markdown.contains("\"A storm\")"));
    }

    #[test]
    fn extract_placeholder_images_drops_overflow_tags_from_markdown() {
        let placeholder_images = extract_placeholder_images(
            "One\n\n<GeneratedImage prompt=\"storm\" alt=\"A storm\" />\n\n<GeneratedImage prompt=\"fog\" alt=\"Fog\" />",
            1,
        )
        .unwrap();

        assert_eq!(placeholder_images.images.len(), 1);
        assert!(!placeholder_images.markdown.contains("<GeneratedImage"));
    }
}
