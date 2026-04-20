mod agent;
mod service;

use axum::extract::{Multipart, Path};
use axum::response::{Html, Redirect};
use axum::routing::post;
use axum::{Form, Router};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

use self::agent::{apply_agent_edit, render_agent_edit_preview, MAX_AGENT_EDIT_REQUEST_CHARS};
use self::service::{apply_article_edit, render_edit_page, require_editable_article};

#[cfg(test)]
use self::agent::{build_unified_diff, markdown_image_count, text_paragraphs};

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

async fn get_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Html<String>, Error> {
    render_edit_page(wr, &slug, MAX_AGENT_EDIT_REQUEST_CHARS).await
}

async fn post_edit_article(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<EditArticleData>,
) -> Result<Redirect, Error> {
    let (auth_user, article) = require_editable_article(&wr, &slug).await?;
    apply_article_edit(&wr, &auth_user, &slug, article, &data, "edit_article", None).await
}

async fn post_agent_edit_preview(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<AgentEditRequestData>,
) -> Result<Html<String>, Error> {
    render_agent_edit_preview(wr, &slug, &data).await
}

async fn post_agent_edit_apply(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Form(data): Form<ApplyAgentEditData>,
) -> Result<Redirect, Error> {
    apply_agent_edit(wr, &slug, data).await
}

async fn post_replace_image(
    wr: WibbleRequest,
    Path((slug, image_id)): Path<(String, String)>,
    multipart: Multipart,
) -> Result<Redirect, Error> {
    service::replace_article_image(wr, &slug, &image_id, multipart).await
}

async fn post_toggle_publish(
    wr: WibbleRequest,
    Path(slug): Path<String>,
) -> Result<Redirect, Error> {
    service::toggle_publish(wr, &slug).await
}

#[cfg(test)]
mod tests {
    use axum::extract::Path;
    use axum::response::Html;
    use axum::Form;
    use sea_orm::{
        ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use crate::entities::{content as content_entity, prelude::AuditLog, prelude::Content};
    use crate::rate_limit::RequesterTier;
    use crate::test_support::{author_user, TestContext};
    use crate::wibble_request::WibbleRequest;

    use super::{build_unified_diff, markdown_image_count, text_paragraphs};

    fn sample_article(author_email: &str) -> content_entity::ActiveModel {
        content_entity::ActiveModel {
            id: ActiveValue::set("story-1".to_string()),
            slug: ActiveValue::set("story-slug".to_string()),
            content: ActiveValue::set(None),
            created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            generating: ActiveValue::set(false),
            generation_started_at: ActiveValue::set(None),
            generation_finished_at: ActiveValue::set(None),
            flagged: ActiveValue::set(false),
            model: ActiveValue::set("test-model".to_string()),
            prompt_version: ActiveValue::set(1),
            fail_count: ActiveValue::set(0),
            description: ActiveValue::set(
                "Officials said the bulletin remained strictly procedural.".to_string(),
            ),
            image_id: ActiveValue::set(None),
            title: ActiveValue::set("Research Bulletin".to_string()),
            user_input: ActiveValue::set("Briefing request".to_string()),
            image_prompt: ActiveValue::set(None),
            user_email: ActiveValue::set(Some(author_email.to_string())),
            votes: ActiveValue::set(7),
            hot_score: ActiveValue::set(0.0),
            generation_time_ms: ActiveValue::set(None),
            flarum_id: ActiveValue::set(None),
            markdown: ActiveValue::set(Some(
                "## Committee Response\n\nThe standing committee accepted the memo without visible alarm.\n\n## Administrative Reply\n\nClerks filed the note without comment.\n\n## Public Desk\n\nCommuters accepted the notice with professional patience.\n\n## Closing Note\n\nOfficials said nothing further."
                    .to_string(),
            )),
            converted: ActiveValue::set(true),
            longview_count: ActiveValue::set(0),
            impression_count: ActiveValue::set(0),
            click_count: ActiveValue::set(0),
            author_email: ActiveValue::set(Some(author_email.to_string())),
            published: ActiveValue::set(false),
            recovered_from_dead_link: ActiveValue::set(false),
        }
    }

    fn sample_request(state: crate::app_state::AppState, email: &str) -> WibbleRequest {
        WibbleRequest {
            state,
            style: "style".to_string(),
            request_path: "/content/story-slug/edit".to_string(),
            auth_user: Some(author_user(email)),
            requester_tier: RequesterTier::Authenticated,
            rate_limit_key: format!("user:{}", email),
            browser_translation_language: None,
            saved_article_language: None,
        }
    }

    async fn spawn_mock_llm_server(response_body: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = vec![0; 16 * 1024];
            let _ = socket.read(&mut buffer).await.unwrap();
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        format!("http://{address}/v1/chat/completions")
    }

    #[test]
    fn markdown_image_count_counts_rendered_article_images() {
        let markdown = "Intro\n\n![One](/image/a \"One\")\n\nBody\n\n![Two](/image/b \"Two\")";

        assert_eq!(markdown_image_count(markdown), 2);
    }

    #[test]
    fn text_paragraphs_skip_image_blocks() {
        let paragraphs = text_paragraphs("One\n\n![Image](/image/a \"A\")\n\nTwo");

        assert_eq!(paragraphs, vec!["One".to_string(), "Two".to_string()]);
    }

    #[test]
    fn unified_diff_includes_labels_and_changed_line() {
        let diff = build_unified_diff("old line", "new line", "before", "after");

        assert!(diff.contains("--- before"));
        assert!(diff.contains("+++ after"));
        assert!(diff.contains("-old line"));
        assert!(diff.contains("+new line"));
    }

    #[tokio::test]
    async fn agent_edit_preview_renders_mocked_revision() {
        let response_body = serde_json::json!({
            "choices": [{
                "message": {
                    "tool_calls": [{
                        "function": {
                            "arguments": serde_json::json!({
                                "title": "Revised Bulletin",
                                "description": "Officials confirmed the memo remained routine.",
                                "markdown": "## Committee Response\n\nThe office said the update was ordinary.\n\n## Administrative Reply\n\nClerks filed the revision without visible alarm.\n\n## Public Desk\n\nCommuters accepted the notice with professional resignation.\n\n## Closing Note\n\nOfficials said nothing further.",
                                "summary": "Tightened the copy and flattened the tone."
                            }).to_string()
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 1,
                "completion_tokens": 1,
                "total_tokens": 2
            }
        })
        .to_string();
        let mock_url = spawn_mock_llm_server(response_body).await;
        let ctx =
            TestContext::new_with_overrides(&[("OPENROUTER_API_URL", mock_url.as_str())]).await;
        sample_article("author@example.com")
            .insert(&ctx.state.db)
            .await
            .unwrap();

        let Html(html) = super::post_agent_edit_preview(
            sample_request(ctx.state.clone(), "author@example.com"),
            Path("story-slug".to_string()),
            Form(super::AgentEditRequestData {
                change_request: "Make it drier".to_string(),
            }),
        )
        .await
        .unwrap();

        assert!(html.contains("Agent edit preview"));
        assert!(html.contains("Tightened the copy and flattened the tone."));
        assert!(html.contains("Revised Bulletin"));
        assert!(html.contains("Prompt version: 1"));
    }

    #[tokio::test]
    async fn agent_edit_apply_updates_article_and_publish_toggle_flips_visibility() {
        let ctx = TestContext::new().await;
        sample_article("author@example.com")
            .insert(&ctx.state.db)
            .await
            .unwrap();

        let _ = super::post_agent_edit_apply(
            sample_request(ctx.state.clone(), "author@example.com"),
            Path("story-slug".to_string()),
            Form(super::ApplyAgentEditData {
                title: "Revised Bulletin".to_string(),
                description: "Officials confirmed the memo remained routine.".to_string(),
                markdown: "## Committee Response\n\nThe office said the update was ordinary.\n\n## Administrative Reply\n\nClerks filed the revision without visible alarm.\n\n## Public Desk\n\nCommuters accepted the notice with professional resignation.\n\n## Closing Note\n\nOfficials said nothing further.".to_string(),
                summary: "Tightened the copy and flattened the tone.".to_string(),
                change_request: "Make it drier".to_string(),
                prompt_version: 1,
            }),
        )
        .await
        .unwrap();

        let updated = Content::find()
            .filter(content_entity::Column::Slug.eq("story-slug"))
            .one(&ctx.state.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.title.trim(), "Revised Bulletin");
        assert_eq!(updated.published, false);

        let _ = super::post_toggle_publish(
            sample_request(ctx.state.clone(), "author@example.com"),
            Path("story-slug".to_string()),
        )
        .await
        .unwrap();

        let published = Content::find()
            .filter(content_entity::Column::Slug.eq("story-slug"))
            .one(&ctx.state.db)
            .await
            .unwrap()
            .unwrap();
        let audit_count = AuditLog::find()
            .filter(crate::entities::audit_log::Column::TargetId.eq("story-slug"))
            .count(&ctx.state.db)
            .await
            .unwrap();

        assert!(published.published);
        assert!(audit_count >= 2);
    }
}
