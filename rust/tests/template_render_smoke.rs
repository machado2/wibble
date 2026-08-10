#![recursion_limit = "256"]

use serde_json::{json, Map, Value};
use tera::{Context, Tera};
use wibble::services::site_text::site_text;

fn render_context(extra: Value) -> Context {
    let mut object = Map::new();
    object.insert("style".to_string(), Value::from("/style.css"));
    object.insert("site_url".to_string(), Value::from("https://example.test"));
    object.insert(
        "canonical_url".to_string(),
        Value::from("https://example.test/current"),
    );
    object.insert("page_language_code".to_string(), Value::from("en"));
    object.insert("page_language_name".to_string(), Value::from("English"));
    object.insert("locale_prefix".to_string(), Value::from(""));
    object.insert("locale_home_url".to_string(), Value::from("/"));
    object.insert("ui".to_string(), site_text().template_strings());
    object.insert(
        "text_create_new_article".to_string(),
        Value::from("Draft article"),
    );
    if let Some(extra_object) = extra.as_object() {
        for (key, value) in extra_object {
            object.insert(key.clone(), value.clone());
        }
    }
    Context::from_serialize(Value::Object(object)).expect("context must serialize")
}

fn render(template_name: &str, extra: Value) -> String {
    let tera = Tera::new("templates/**/*").expect("templates should load");
    tera.render(template_name, &render_context(extra))
        .unwrap_or_else(|err| panic!("failed to render {}: {}", template_name, err))
}

#[test]
fn create_template_renders_single_prompt_form() {
    let html = render(
        "create.html",
        json!({
            "title": "Create",
            "description": "Create flow",
            "robots": "noindex,nofollow",
            "prompt": "Prompt",
            "prompt_max_length": 600,
            "prompt_presets": [
                {"label": "Policy", "prompt": "Prompt body"}
            ],
            "logged_in": true,
            "selected_create_mode": "research",
            "standard_quota": {"hourly": 20, "daily": 40},
            "research_quota": {"hourly": 5, "daily": 10},
            "authenticated_standard_quota": {"hourly": 20, "daily": 40},
            "authenticated_research_quota": {"hourly": 5, "daily": 10},
            "authenticated_edit_quota": {"hourly": 10, "daily": 20},
            "owner_editing_note": "Owner editing is capped separately at 10 edit-agent previews per hour.",
            "research_lane_note": "Research-backed filings run on their own desk at 5 per hour / 10 per day.",
            "login_upsell_note": "Login raises the standard desk to 20 per hour, opens a bounded research desk at 5 per hour, keeps results private as drafts, and unlocks the edit desk.",
            "research_quota_note": "Separate quota: 5 per hour / 10 per day."
        }),
    );

    assert!(html.contains("Enter your prompt"));
    assert!(html.contains("Enter your prompt / hint / instructions here..."));
    assert!(!html.contains("Research desk"));
}

#[test]
fn wait_template_renders_simple_generation_state() {
    let html = render(
        "wait.html",
        json!({
            "title": "Wait",
            "description": "Wait flow",
            "robots": "noindex,nofollow",
            "id": "job-1",
            "wait_auto_refresh": false,
            "wait_summary": {
                "article_title": "Incident bulletin",
                "slug": "incident-bulletin",
                "stage_title": "Waiting for clarification",
                "stage_description": "The draft is paused pending an answer.",
                "publication_title": "Destination: draft",
                "publication_note": "Signed-in articles stay private until review.",
                "image_total": 0,
                "image_completed": 0,
                "image_processing": 0,
                "image_failed": 0,
                "clarification_question": "Which ministry issued the notice?",
                "clarification_deadline": "2026-04-20 12:00",
                "clarification_deadline_note": "If nobody answers by 2026-04-20 12:00, the job resumes with a conservative fallback.",
                "phase_items": [
                    {"label": "Queued", "state": "done"},
                    {"label": "Clarify", "state": "active"},
                    {"label": "Write", "state": "pending"}
                ]
            }
        }),
    );

    assert!(html.contains("Generating article..."));
    assert!(html.contains("Incident bulletin"));
    assert!(!html.contains("Which ministry issued the notice?"));
}

#[test]
fn edit_preview_template_renders_diff_and_apply_form() {
    let html = render(
        "edit_agent_preview.html",
        json!({
            "title": "Preview",
            "description": "Edit preview",
            "robots": "noindex,nofollow",
            "slug": "story-slug",
            "change_request": "Make it drier",
            "summary": "Tightened the opening and cut repetition.",
            "prompt_version": 1,
            "current_title": "Current title",
            "current_description": "Current description",
            "current_markdown": "Current markdown",
            "proposed_title": "Proposed title",
            "proposed_description": "Proposed description",
            "proposed_markdown": "Proposed markdown",
            "title_diff": "--- current\n+++ proposed\n",
            "description_diff": "--- current\n+++ proposed\n",
            "markdown_diff": "--- current\n+++ proposed\n"
        }),
    );

    assert!(html.contains("Agent edit preview"));
    assert!(html.contains("Apply agent revision"));
    assert!(html.contains("Prompt version: 1"));
}

#[test]
fn content_template_renders_plain_article_without_translation_ui() {
    let html = render(
        "content.html",
        json!({
            "title": "Filed report",
            "description": "Desc",
            "slug": "story-slug",
            "id": "story-id",
            "created_at": "2026-04-19",
            "image_id": "img-1",
            "body": "<p>Body</p>",
            "page_language_code": "en",
            "page_language_name": "English",
            "article_source_language_code": "en",
            "article_source_language_name": "English",
            "article_research_metadata_present": true,
            "article_research_metadata": {
                "mode_label": "Requested research desk",
                "source_count": 3
            },
            "article_research_note": "Requested research desk. This filing was grounded against 3 public-source briefs before drafting. The source trace is kept off-page so the article body stays deadpan.",
            "can_edit": true,
            "can_publish": true,
            "is_published": false,
            "vote_score": 7,
            "voting_open": true,
            "can_vote": true,
            "user_vote": "up",
            "comments": [],
            "comment_count": 0,
            "comment_count_label": "0 comments",
            "comments_open": true,
            "can_comment": true,
            "comment_pager": {
                "total_pages": 1,
                "current_page": 1,
                "has_prev": false,
                "has_next": false,
                "prev_page": 1,
                "next_page": 1
            },
            "comment_page_label": "Page 1 / 1"
            ,"is_admin": true
        }),
    );

    assert!(html.contains("Filed report"));
    assert!(html.contains("<p>Body</p>"));
    assert!(html.contains("/content/story-slug/edit"));
    assert!(!html.contains("Requested research desk"));
    assert!(!html.contains("Comments"));
}
