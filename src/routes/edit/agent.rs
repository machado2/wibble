use axum::response::{Html, Redirect};
use similar::TextDiff;

use crate::error::Error;
use crate::llm::article_generator::{
    ensure_minimum_paragraph_count, split_paragraphs, validate_article_output,
};
use crate::llm::edit_agent::generate_edit_proposal;
use crate::services::editorial_policy::enforce_edit_request_policy;
use crate::wibble_request::WibbleRequest;

use super::service::{apply_article_edit, require_editable_article};

pub(super) const MAX_AGENT_EDIT_REQUEST_CHARS: usize = 400;

pub(super) fn normalize_agent_edit_request(raw: &str) -> Result<String, Error> {
    let request = raw.trim();
    if request.is_empty() {
        return Err(Error::BadRequest(
            "Describe the change before asking the edit agent to revise the article.".to_string(),
        ));
    }
    if request.chars().count() > MAX_AGENT_EDIT_REQUEST_CHARS {
        return Err(Error::BadRequest(format!(
            "Agent edit request is too long. Keep it under {} characters.",
            MAX_AGENT_EDIT_REQUEST_CHARS
        )));
    }
    enforce_edit_request_policy(request)?;
    Ok(request.to_string())
}

pub(super) fn markdown_image_count(markdown: &str) -> usize {
    markdown.matches("](/image/").count()
}

pub(super) fn text_paragraphs(markdown: &str) -> Vec<String> {
    split_paragraphs(markdown)
        .into_iter()
        .filter(|paragraph| !paragraph.trim().starts_with("!["))
        .collect()
}

pub(super) fn build_unified_diff(
    before: &str,
    after: &str,
    before_label: &str,
    after_label: &str,
) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(1)
        .header(before_label, after_label)
        .to_string()
}

pub(super) async fn render_agent_edit_preview(
    wr: WibbleRequest,
    slug: &str,
    data: &super::AgentEditRequestData,
) -> Result<Html<String>, Error> {
    let text = wr.site_text();
    let change_request = normalize_agent_edit_request(&data.change_request)?;
    let (auth_user, article) = require_editable_article(&wr, slug).await?;
    let current_markdown = article.markdown.as_deref().unwrap_or("");
    let expected_images = markdown_image_count(current_markdown);
    let model = if article.model.trim().is_empty() {
        wr.state
            .llm
            .models
            .first()
            .ok_or_else(|| Error::Llm("No language model configured".to_string()))?
            .as_str()
    } else {
        article.model.as_str()
    };

    let proposal = generate_edit_proposal(
        &wr.state.llm,
        model,
        &article.title,
        &article.description,
        current_markdown,
        &change_request,
    )
    .await?;

    ensure_minimum_paragraph_count(&text_paragraphs(&proposal.markdown))?;
    validate_article_output(&proposal.title, &proposal.markdown, expected_images)?;

    let preview_details = serde_json::json!({
        "change_request": change_request,
        "summary": proposal.summary,
        "prompt_version": proposal.prompt_version,
    })
    .to_string();
    crate::audit::log_audit(
        &wr.state.db,
        &auth_user,
        "agent_edit_preview",
        "content",
        slug,
        Some(preview_details),
    )
    .await?;

    wr.template("edit_agent_preview")
        .await
        .insert("title", &text.edit_preview_meta_title(&article.title))
        .insert("robots", "noindex,nofollow")
        .insert("slug", slug)
        .insert("change_request", &change_request)
        .insert("summary", &proposal.summary)
        .insert("prompt_version", &proposal.prompt_version)
        .insert("current_title", &article.title)
        .insert("current_description", &article.description)
        .insert("current_markdown", current_markdown)
        .insert("proposed_title", &proposal.title)
        .insert("proposed_description", &proposal.description)
        .insert("proposed_markdown", &proposal.markdown)
        .insert(
            "title_diff",
            &build_unified_diff(
                &article.title,
                &proposal.title,
                if text.language().code == "pt" {
                    "título atual"
                } else {
                    "current title"
                },
                if text.language().code == "pt" {
                    "título proposto"
                } else {
                    "proposed title"
                },
            ),
        )
        .insert(
            "description_diff",
            &build_unified_diff(
                &article.description,
                &proposal.description,
                if text.language().code == "pt" {
                    "descrição atual"
                } else {
                    "current description"
                },
                if text.language().code == "pt" {
                    "descrição proposta"
                } else {
                    "proposed description"
                },
            ),
        )
        .insert(
            "markdown_diff",
            &build_unified_diff(
                current_markdown,
                &proposal.markdown,
                if text.language().code == "pt" {
                    "markdown atual"
                } else {
                    "current markdown"
                },
                if text.language().code == "pt" {
                    "markdown proposto"
                } else {
                    "proposed markdown"
                },
            ),
        )
        .render()
}

pub(super) async fn apply_agent_edit(
    wr: WibbleRequest,
    slug: &str,
    data: super::ApplyAgentEditData,
) -> Result<Redirect, Error> {
    let change_request = normalize_agent_edit_request(&data.change_request)?;
    let summary = data.summary.trim().to_string();
    if summary.is_empty() {
        return Err(Error::BadRequest(
            "Agent edit summary is missing from the preview payload.".to_string(),
        ));
    }

    let (auth_user, article) = require_editable_article(&wr, slug).await?;
    let expected_images = markdown_image_count(article.markdown.as_deref().unwrap_or(""));
    ensure_minimum_paragraph_count(&text_paragraphs(&data.markdown))?;
    validate_article_output(&data.title, &data.markdown, expected_images)?;

    let audit_details = serde_json::json!({
        "change_request": change_request,
        "summary": summary,
        "prompt_version": data.prompt_version,
    })
    .to_string();

    apply_article_edit(
        &wr,
        &auth_user,
        slug,
        article,
        &super::EditArticleData {
            title: data.title,
            description: data.description,
            markdown: data.markdown,
        },
        "agent_edit_apply",
        Some(audit_details),
    )
    .await
}
