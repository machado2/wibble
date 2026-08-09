use axum::response::Html;

use crate::error::Error;
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::wibble_request::WibbleRequest;

use super::{query, render};

pub(super) async fn render_content_page(
    request: &WibbleRequest,
    slug: &str,
) -> Result<Html<String>, Error> {
    let article = match query::load_content_page_article(request, slug).await? {
        query::ContentPageArticle::Ready(article) => *article,
        query::ContentPageArticle::Wait(wait_page) => return Ok(wait_page),
    };

    let image_id = article.image_id.clone().unwrap_or_default();
    let markdown = article.markdown.as_deref().ok_or_else(|| {
        Error::NotFound(Some(format!(
            "Markdown for content {} not found",
            article.id
        )))
    })?;
    let body = render::markdown_to_html(
        &render::strip_leading_description(markdown, &article.description),
        "",
    );
    let mut template = request.template("content").await;
    template
        .insert("id", &article.id)
        .insert("slug", &article.slug)
        .insert("created_at", &article.created_at.format("%F").to_string())
        .insert("description", &article.description)
        .insert("image_id", &image_id)
        .insert("title", &article.title)
        .insert("body", &body)
        .insert("is_published", &article.published)
        .insert(
            "can_edit",
            &request
                .auth_user
                .as_ref()
                .is_some_and(|u| can_edit_article(u, &article)),
        )
        .insert(
            "can_publish",
            &request
                .auth_user
                .as_ref()
                .is_some_and(|u| can_toggle_publish(u, &article)),
        );
    if !article.published || article.flagged {
        template.insert("robots", "noindex,nofollow");
    }
    template.render()
}
