use crate::error::Error;
use crate::image_generator::{ImageGenerated, ImageToCreate};
use crate::services::editorial_policy::enforce_article_output_policy;

use super::research::ResearchSource;

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

pub fn ensure_title_present(title: &str) -> Result<(), Error> {
    if title.trim().is_empty() {
        return Err(Error::Llm("Generated article title is empty".to_string()));
    }
    Ok(())
}

pub fn ensure_no_forbidden_markup(markdown: &str) -> Result<(), Error> {
    let normalized = markdown.to_ascii_lowercase();
    for forbidden in [
        "<script",
        "<iframe",
        "<object",
        "<embed",
        "<form",
        "<style",
        "<generatedimage",
    ] {
        if normalized.contains(forbidden) {
            return Err(Error::Llm(format!(
                "Generated article contains forbidden markup: {}",
                forbidden
            )));
        }
    }
    Ok(())
}

pub fn ensure_no_prompt_leakage(text: &str) -> Result<(), Error> {
    let normalized = text.to_ascii_lowercase();
    for leaked_phrase in [
        "as an ai",
        "language model",
        "system prompt",
        "user prompt",
        "these instructions",
        "i cannot comply",
    ] {
        if normalized.contains(leaked_phrase) {
            return Err(Error::Llm(format!(
                "Generated article leaked prompt scaffolding: {}",
                leaked_phrase
            )));
        }
    }
    Ok(())
}

pub fn ensure_no_research_citation_scaffolding(text: &str) -> Result<(), Error> {
    let normalized = text.to_ascii_lowercase();
    for forbidden in [
        "http://",
        "https://",
        "www.",
        "[1]",
        "[2]",
        "research file",
        "research notes",
        "source list",
        "footnote",
        "citation",
    ] {
        if normalized.contains(forbidden) {
            return Err(Error::Llm(format!(
                "Generated researched article leaked citation scaffolding: {}",
                forbidden
            )));
        }
    }
    Ok(())
}

pub fn ensure_no_internal_source_domain_mentions(
    text: &str,
    sources: &[ResearchSource],
) -> Result<(), Error> {
    let normalized = text.to_ascii_lowercase();
    for source in sources {
        let domain = source.domain.trim().to_ascii_lowercase();
        for marker in source_leak_markers(&domain) {
            if normalized.contains(&marker) {
                return Err(Error::Llm(format!(
                    "Generated researched article cited an internal source domain: {}",
                    marker
                )));
            }
        }
    }
    Ok(())
}

fn source_leak_markers(domain: &str) -> Vec<String> {
    if domain.is_empty() {
        return Vec::new();
    }
    let mut markers = vec![domain.to_string()];
    for publisher in [
        "reuters",
        "apnews",
        "associated press",
        "bbc",
        "npr",
        "economist",
    ] {
        if domain.contains(publisher) {
            markers.push(publisher.to_string());
        }
    }
    if domain.contains("ft.com") {
        markers.push("financial times".to_string());
    }
    markers.sort();
    markers.dedup();
    markers
}

pub fn ensure_deadpan_tone(title: &str, markdown: &str) -> Result<(), Error> {
    let normalized = format!("{}\n{}", title, markdown).to_ascii_lowercase();
    for tone_break in ["haha", "lol", "lmao", "this is satire", "this is parody"] {
        if normalized.contains(tone_break) {
            return Err(Error::Llm(format!(
                "Generated article broke deadpan tone: {}",
                tone_break
            )));
        }
    }
    Ok(())
}

pub fn ensure_image_markdown_count(markdown: &str, expected_images: usize) -> Result<(), Error> {
    let actual_images = markdown.matches("](/image/").count();
    if actual_images != expected_images {
        return Err(Error::Llm(format!(
            "Generated article image count mismatch: expected {}, found {}",
            expected_images, actual_images
        )));
    }
    Ok(())
}

pub fn validate_article_output(
    title: &str,
    markdown: &str,
    expected_images: usize,
) -> Result<(), Error> {
    ensure_title_present(title)?;
    ensure_no_prompt_leakage(title)?;
    ensure_no_prompt_leakage(markdown)?;
    ensure_deadpan_tone(title, markdown)?;
    ensure_no_forbidden_markup(markdown)?;
    ensure_image_markdown_count(markdown, expected_images)?;
    enforce_article_output_policy(title, "", markdown)?;
    Ok(())
}

pub fn validate_researched_article_output(
    title: &str,
    markdown: &str,
    expected_images: usize,
    sources: &[ResearchSource],
) -> Result<(), Error> {
    validate_article_output(title, markdown, expected_images)?;
    let full_text = format!("{}\n{}", title, markdown);
    ensure_no_research_citation_scaffolding(&full_text)?;
    ensure_no_internal_source_domain_mentions(&full_text, sources)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_minimum_paragraph_count, ensure_no_forbidden_markup,
        ensure_no_internal_source_domain_mentions, ensure_no_prompt_leakage,
        ensure_no_research_citation_scaffolding, parse_titled_markdown, source_leak_markers,
        split_paragraphs, validate_article_output,
    };
    use crate::llm::article_generator::research::ResearchSource;

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

    #[test]
    fn prompt_leakage_validation_rejects_meta_phrases() {
        let err = ensure_no_prompt_leakage("As an AI language model, I regret")
            .unwrap_err()
            .to_string();

        assert!(err.contains("prompt scaffolding"));
    }

    #[test]
    fn forbidden_markup_validation_rejects_placeholder_tags() {
        let err = ensure_no_forbidden_markup(
            "Paragraph\n\n<GeneratedImage prompt=\"storm\" alt=\"Storm\" />",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("forbidden markup"));
    }

    #[test]
    fn validate_article_output_rejects_wrong_image_count() {
        let err = validate_article_output("Headline", "Paragraph\n\n![Image](/image/a \"A\")", 2)
            .unwrap_err()
            .to_string();

        assert!(err.contains("image count mismatch"));
    }

    #[test]
    fn researched_output_rejects_citation_scaffolding() {
        let err = ensure_no_research_citation_scaffolding("See https://example.com [1]")
            .unwrap_err()
            .to_string();

        assert!(err.contains("citation scaffolding"));
    }

    #[test]
    fn researched_output_rejects_internal_source_domains() {
        let err = ensure_no_internal_source_domain_mentions(
            "Officials said Reuters had framed the matter carefully.",
            &[ResearchSource {
                title: "Wire update".to_string(),
                url: "https://www.reuters.com/example".to_string(),
                domain: "reuters.com".to_string(),
                snippet: String::new(),
                context: String::new(),
            }],
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("internal source domain"));
    }

    #[test]
    fn source_leak_markers_include_known_publication_names() {
        let markers = source_leak_markers("reuters.com");

        assert!(markers.contains(&"reuters.com".to_string()));
        assert!(markers.contains(&"reuters".to_string()));
    }
}
