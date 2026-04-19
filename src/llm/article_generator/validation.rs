use crate::error::Error;
use crate::image_generator::{ImageGenerated, ImageToCreate};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedArticleDraft {
    pub title: String,
    pub body: String,
}

pub fn parse_titled_markdown(markdown: &str) -> Result<ParsedArticleDraft, Error> {
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
        let body = lines.collect::<Vec<&str>>().join("\n");
        return Ok(ParsedArticleDraft {
            title: title.to_string(),
            body,
        });
    }

    Err(Error::Llm("No title found in article".to_string()))
}

pub fn split_paragraphs(body: &str) -> Vec<String> {
    body.split("\n\n")
        .map(|paragraph| paragraph.to_string())
        .collect()
}

pub fn ensure_minimum_paragraph_count(paragraphs: &[String]) -> Result<(), Error> {
    if paragraphs.len() < 4 {
        return Err(Error::Llm("Article has less than 4 paragraphs".to_string()));
    }
    Ok(())
}

pub fn ensure_image_briefs_present(images: &[ImageToCreate]) -> Result<(), Error> {
    if images.is_empty() {
        return Err(Error::Llm("Failed to generate image prompts".to_string()));
    }
    Ok(())
}

pub fn ensure_placeholder_images_present(images: &[ImageToCreate]) -> Result<(), Error> {
    if images.is_empty() {
        return Err(Error::ImageGeneration(
            "No image placeholders found in generated article".into(),
        ));
    }
    Ok(())
}

pub fn ensure_generated_images_present(images: &[ImageGenerated]) -> Result<(), Error> {
    if images.is_empty() {
        return Err(Error::ImageGeneration(
            "All image generations failed".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ensure_minimum_paragraph_count, parse_titled_markdown, split_paragraphs};

    #[test]
    fn parse_titled_markdown_extracts_heading_and_body() {
        let parsed = parse_titled_markdown("\n# Headline\n\nBody line").unwrap();

        assert_eq!(parsed.title, "Headline");
        assert_eq!(parsed.body, "\nBody line");
    }

    #[test]
    fn parse_titled_markdown_supports_title_prefix() {
        let parsed = parse_titled_markdown("Title: Cabinet Falls\n\nBody").unwrap();

        assert_eq!(parsed.title, "Cabinet Falls");
    }

    #[test]
    fn split_paragraphs_keeps_paragraph_order() {
        let paragraphs = split_paragraphs("One\n\nTwo\n\nThree");

        assert_eq!(paragraphs, vec!["One", "Two", "Three"]);
    }

    #[test]
    fn ensure_minimum_paragraph_count_rejects_short_articles() {
        let err = ensure_minimum_paragraph_count(&["One".to_string(), "Two".to_string()])
            .unwrap_err()
            .to_string();

        assert!(err.contains("less than 4 paragraphs"));
    }
}
