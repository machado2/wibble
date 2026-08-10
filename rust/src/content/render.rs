use std::sync::OnceLock;

use markdown::{to_html, to_html_with_options, Options};
use regex::Regex;

fn article_image_regex() -> &'static Regex {
    static ARTICLE_IMAGE_REGEX: OnceLock<Regex> = OnceLock::new();
    ARTICLE_IMAGE_REGEX.get_or_init(|| {
        Regex::new(r#"<img src="(/image/([^"/?#]+))"([^>]*) />"#)
            .expect("article image regex must compile")
    })
}

fn link_article_images(html: &str, locale_prefix: &str) -> String {
    article_image_regex()
        .replace_all(html, |caps: &regex::Captures<'_>| {
            format!(
                r#"<a href="{locale_prefix}/image_info/{id}" class="article-image-link">{img}</a>"#,
                locale_prefix = locale_prefix,
                id = &caps[2],
                img = &caps[0],
            )
        })
        .into_owned()
}

pub fn markdown_to_html(markdown_str: &str, locale_prefix: &str) -> String {
    let html = to_html_with_options(markdown_str, &Options::gfm())
        .unwrap_or_else(|_| to_html(markdown_str));
    link_article_images(&html, locale_prefix)
}

pub fn strip_leading_description(markdown: &str, description: &str) -> String {
    let markdown = markdown.trim();
    let description = description.trim();
    if description.is_empty() {
        return markdown.to_string();
    }

    let mut parts = markdown.splitn(2, "\n\n");
    let first_block = parts.next().unwrap_or("").trim();
    if first_block == description {
        parts.next().unwrap_or("").trim().to_string()
    } else {
        markdown.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{markdown_to_html, strip_leading_description};

    #[test]
    fn markdown_rendering_escapes_raw_html_and_wraps_article_images() {
        let rendered = markdown_to_html(
            r#"<script>alert(1)</script>

![Alt text](/image/abc-123 "Prompt")
"#,
            "/pt",
        );

        assert!(!rendered.contains("<script>"));
        assert!(rendered.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(rendered.contains(r#"href="/pt/image_info/abc-123""#));
        assert!(rendered.contains(r#"src="/image/abc-123""#));
    }

    #[test]
    fn strips_duplicate_standfirst_from_article_body() {
        let markdown = "Opening paragraph.\n\n## Section\n\nMore detail.";
        assert_eq!(
            strip_leading_description(markdown, "Opening paragraph."),
            "## Section\n\nMore detail."
        );
        assert_eq!(
            strip_leading_description(markdown, "Something else"),
            markdown
        );
    }
}
