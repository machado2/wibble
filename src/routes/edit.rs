use axum::extract::{Multipart, Path};
use axum::response::{Html, Redirect};
use axum::routing::post;
use axum::{Form, Router};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::Deserialize;
use similar::TextDiff;

use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::entities::{content as content_entity, content_image, prelude::*};
use crate::error::Error;
use crate::llm::article_generator::{
    ensure_minimum_paragraph_count, split_paragraphs, validate_article_output,
};
use crate::llm::edit_agent::generate_edit_proposal;
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::repositories::images::store_image_file;
use crate::services::article_translations::owned_article_source_text;
use crate::translation_jobs::refresh_article_translations_after_edit;
use crate::wibble_request::WibbleRequest;

const MAX_AGENT_EDIT_REQUEST_CHARS: usize = 400;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/content/{slug}/edit",
            axum::routing::get(get_edit_article).post(post_edit_article),
        )
        .route("/content/{slug}/edit/agent", post(post_agent_edit_preview))
        .route(
            "/content/{slug}/edit/agent/apply",
            post(post_agent_edit_apply),
        )
        .route(
            "/content/{slug}/images/{image_id}",
            post(post_replace_image),
        )
        .route("/content/{slug}/publish", post(post_toggle_publish))
}

#[derive(Deserialize, Debug)]
struct EditArticleData {
    title: String,
    description: String,
    markdown: String,
}

#[derive(Deserialize, Debug)]
struct AgentEditRequestData {
    change_request: String,
}

#[derive(Deserialize, Debug)]
struct ApplyAgentEditData {
    title: String,
    description: String,
    markdown: String,
    summary: String,
    change_request: String,
    prompt_version: i32,
}

fn normalize_agent_edit_request(raw: &str) -> Result<String, Error> {
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
    Ok(request.to_string())
}

fn markdown_image_count(markdown: &str) -> usize {
    markdown.matches("](/image/").count()
}

fn text_paragraphs(markdown: &str) -> Vec<String> {
    split_paragraphs(markdown)
        .into_iter()
        .filter(|paragraph| !paragraph.trim().starts_with("!["))
        .collect()
}

fn build_unified_diff(before: &str, after: &str, before_label: &str, after_label: &str) -> String {
    TextDiff::from_lines(before, after)
        .unified_diff()
        .context_radius(1)
        .header(before_label, after_label)
        .to_string()
}

async fn require_editable_article(
    wr: &WibbleRequest,
    slug: &str,
) -> Result<(AuthUser, content_entity::Model), Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?
        .clone();
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(slug))
        .one(&wr.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or_else(|| Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_edit_article(&auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to edit this article".to_string(),
        ));
    }

    Ok((auth_user, article))
}

async fn apply_article_edit(
    wr: &WibbleRequest,
    auth_user: &AuthUser,
    slug: &str,
    article: content_entity::Model,
    data: EditArticleData,
    audit_action: &str,
    audit_details: Option<String>,
) -> Result<Redirect, Error> {
    let db = &wr.state.db;
    let previous_source = owned_article_source_text(&article);
    let translatable_content_changed = article.title != data.title
        || article.description != data.description
        || article.markdown.as_deref().unwrap_or("") != data.markdown;
    let article_id = article.id.clone();

    let mut active: content_entity::ActiveModel = article.into();
    active.title = ActiveValue::set(data.title.clone());
    active.description = ActiveValue::set(data.description.clone());
    active.markdown = ActiveValue::set(Some(data.markdown.clone()));
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating article: {}", e)))?;

    log_audit(db, auth_user, audit_action, "content", slug, audit_details).await?;
    if translatable_content_changed {
        if let Some(previous_source) = previous_source {
            refresh_article_translations_after_edit(
                wr.state.clone(),
                auth_user,
                slug,
                previous_source,
                crate::services::article_translations::OwnedArticleSourceText {
                    article_id,
                    title: data.title,
                    description: data.description,
                    markdown: data.markdown,
                },
            )
            .await?;
        }
    }

    Ok(Redirect::to(&format!("/content/{}", slug)))
}

async fn get_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Html<String>, Error> {
    let (_auth_user, article) = require_editable_article(&wr, &slug).await?;
    let images = ContentImage::find()
        .filter(content_image::Column::ContentId.eq(&article.id))
        .all(&wr.state.db)
        .await
        .map_err(|e| Error::Database(format!("Error loading images: {}", e)))?;

    let image_data: Vec<_> = images
        .iter()
        .map(|img| {
            serde_json::json!({
                "id": img.id,
                "alt_text": img.alt_text,
                "prompt": img.prompt,
            })
        })
        .collect();

    wr.template("edit")
        .await
        .insert("title", &format!("Edit: {}", article.title))
        .insert("robots", "noindex,nofollow")
        .insert("article_title", &article.title)
        .insert("article_description", &article.description)
        .insert(
            "article_markdown",
            article.markdown.as_deref().unwrap_or(""),
        )
        .insert("slug", &slug)
        .insert("id", &article.id)
        .insert("images", &image_data)
        .insert("agent_edit_max_length", &MAX_AGENT_EDIT_REQUEST_CHARS)
        .render()
}

async fn post_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<EditArticleData>,
) -> Result<Redirect, Error> {
    let (auth_user, article) = require_editable_article(&wr, &slug).await?;
    apply_article_edit(&wr, &auth_user, &slug, article, data, "edit_article", None).await
}

async fn post_agent_edit_preview(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<AgentEditRequestData>,
) -> Result<Html<String>, Error> {
    let change_request = normalize_agent_edit_request(&data.change_request)?;
    let (auth_user, article) = require_editable_article(&wr, &slug).await?;
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
    log_audit(
        &wr.state.db,
        &auth_user,
        "agent_edit_preview",
        "content",
        &slug,
        Some(preview_details),
    )
    .await?;

    wr.template("edit_agent_preview")
        .await
        .insert("title", &format!("Agent edit preview: {}", article.title))
        .insert("robots", "noindex,nofollow")
        .insert("slug", &slug)
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
                "current title",
                "proposed title",
            ),
        )
        .insert(
            "description_diff",
            &build_unified_diff(
                &article.description,
                &proposal.description,
                "current description",
                "proposed description",
            ),
        )
        .insert(
            "markdown_diff",
            &build_unified_diff(
                current_markdown,
                &proposal.markdown,
                "current markdown",
                "proposed markdown",
            ),
        )
        .render()
}

async fn post_agent_edit_apply(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<ApplyAgentEditData>,
) -> Result<Redirect, Error> {
    let change_request = normalize_agent_edit_request(&data.change_request)?;
    let summary = data.summary.trim().to_string();
    if summary.is_empty() {
        return Err(Error::BadRequest(
            "Agent edit summary is missing from the preview payload.".to_string(),
        ));
    }

    let (auth_user, article) = require_editable_article(&wr, &slug).await?;
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
        &slug,
        article,
        EditArticleData {
            title: data.title,
            description: data.description,
            markdown: data.markdown,
        },
        "agent_edit_apply",
        Some(audit_details),
    )
    .await
}

async fn post_replace_image(
    wr: WibbleRequest,
    Path((slug, image_id)): Path<(String, String)>,
    mut multipart: Multipart,
) -> Result<Redirect, Error> {
    let (auth_user, article) = require_editable_article(&wr, &slug).await?;
    let db = &wr.state.db;

    let img = ContentImage::find_by_id(image_id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding image: {}", e)))?
        .ok_or_else(|| Error::NotFound(Some(format!("Image {} not found", image_id))))?;

    if img.content_id != article.id {
        return Err(Error::Auth(
            "Image does not belong to this article".to_string(),
        ));
    }

    let mut image_data = None;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name().unwrap_or("") == "image" {
            let data = field
                .bytes()
                .await
                .map_err(|e| Error::Auth(format!("Failed to read upload: {}", e)))?;
            image_data = Some(data.to_vec());
        }
    }

    let image_data = image_data.ok_or_else(|| Error::Auth("No image uploaded".to_string()))?;
    store_image_file(&image_id, image_data).await?;

    log_audit(
        db,
        &auth_user,
        "replace_image",
        "content_image",
        &image_id,
        Some(format!("article={}", slug)),
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}/edit", slug)))
}

async fn post_toggle_publish(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Redirect, Error> {
    let auth_user = wr
        .auth_user
        .as_ref()
        .ok_or_else(|| Error::Auth("Login required".to_string()))?;
    let db = &wr.state.db;
    let article = Content::find()
        .filter(content_entity::Column::Slug.eq(&slug))
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error finding article: {}", e)))?
        .ok_or_else(|| Error::NotFound(Some(format!("Article {} not found", slug))))?;

    if !can_toggle_publish(auth_user, &article) {
        return Err(Error::Auth(
            "Not authorized to toggle publish state".to_string(),
        ));
    }

    let new_state = !article.published;
    let mut active: content_entity::ActiveModel = article.into();
    active.published = ActiveValue::set(new_state);
    active
        .update(db)
        .await
        .map_err(|e| Error::Database(format!("Error updating publish state: {}", e)))?;

    log_audit(
        db,
        auth_user,
        if new_state {
            "publish_article"
        } else {
            "unpublish_article"
        },
        "content",
        &slug,
        None,
    )
    .await?;

    Ok(Redirect::to(&format!("/content/{}", slug)))
}
