use crate::image_generator::ImageGenerated;

pub fn compose_article_markdown(paragraphs: Vec<String>, images: &[ImageGenerated]) -> String {
    let mut remaining_images = images.iter();
    let mut markdown = Vec::<String>::new();
    let mut paragraphs_per_image = if images.is_empty() {
        paragraphs.len()
    } else {
        paragraphs.len() / images.len()
    };
    if paragraphs_per_image == 0 {
        paragraphs_per_image = 1;
    }

    let mut paragraph_index = 0;
    for paragraph in paragraphs {
        markdown.push(paragraph);
        paragraph_index += 1;

        if paragraph_index % paragraphs_per_image == 0 {
            if let Some(image) = remaining_images.next() {
                markdown.push(render_generated_image_markdown(image));
            }
        }
    }

    for image in remaining_images {
        markdown.push(render_generated_image_markdown(image));
    }

    markdown.join("\n\n")
}

pub fn leading_paragraph(markdown: &str) -> String {
    markdown.split("\n\n").next().unwrap_or("").to_string()
}

fn render_generated_image_markdown(image: &ImageGenerated) -> String {
    format!(
        "![{}](/image/{} \"{}\")",
        image.img.caption, image.id, image.img.caption
    )
}

#[cfg(test)]
mod tests {
    use crate::image_generator::{ImageGenerated, ImageToCreate};

    use super::{compose_article_markdown, leading_paragraph};

    fn sample_image(id: &str, caption: &str) -> ImageGenerated {
        ImageGenerated {
            id: id.to_string(),
            img: ImageToCreate {
                id: id.to_string(),
                caption: caption.to_string(),
                prompt: format!("prompt-{}", id),
            },
            data: Vec::new(),
            parameters: String::new(),
        }
    }

    #[test]
    fn compose_article_markdown_interleaves_images_through_paragraphs() {
        let markdown = compose_article_markdown(
            vec![
                "Paragraph 1".to_string(),
                "Paragraph 2".to_string(),
                "Paragraph 3".to_string(),
                "Paragraph 4".to_string(),
            ],
            &[
                sample_image("img-1", "Image 1"),
                sample_image("img-2", "Image 2"),
            ],
        );

        assert!(markdown.contains("Paragraph 1\n\nParagraph 2\n\n![Image 1](/image/img-1"));
        assert!(markdown.contains("Paragraph 3\n\nParagraph 4\n\n![Image 2](/image/img-2"));
    }

    #[test]
    fn leading_paragraph_preserves_first_block() {
        assert_eq!(leading_paragraph("\n\nLead\n\nBody"), "");
    }
}
