use serde_json::{json, Map, Value};
use tera::{Context, Tera};

fn render_context(extra: Value) -> Context {
    let mut object = Map::new();
    object.insert("style".to_string(), Value::from("/style.css"));
    object.insert("site_url".to_string(), Value::from("https://example.test"));
    object.insert(
        "canonical_url".to_string(),
        Value::from("https://example.test/current"),
    );
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
fn create_template_renders_research_mode_form() {
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
            "authenticated_translation_quota": {"hourly": 40, "daily": 100}
        }),
    );

    assert!(html.contains("Desk mode"));
    assert!(html.contains("Research desk"));
    assert!(html.contains("mode-research"));
    assert!(html.contains("checked"));
}

#[test]
fn wait_template_renders_clarification_state() {
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
                "phase_items": [
                    {"label": "Queued", "state": "done"},
                    {"label": "Clarify", "state": "active"},
                    {"label": "Write", "state": "pending"}
                ]
            }
        }),
    );

    assert!(html.contains("Clarification needed"));
    assert!(html.contains("Resume drafting"));
    assert!(html.contains("Which ministry issued the notice?"));
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
fn content_template_renders_research_and_language_metadata() {
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
            "preferred_article_language_code": "en",
            "preferred_article_language_name": "English",
            "preferred_article_language_source": "source",
            "served_article_language_source": "preferred",
            "article_translation_requested": false,
            "article_translation_available": true,
            "article_language_options": [
                {
                    "href": "/content/story-slug?lang=auto",
                    "label": "Automatic",
                    "note": "Original edition: English",
                    "active": true
                }
            ],
            "article_language_menu_open": false,
            "article_research_metadata_present": true,
            "article_research_metadata": {
                "mode_label": "Requested research desk",
                "source_count": 3
            },
            "can_edit": true,
            "can_publish": true,
            "is_published": false,
            "vote_score": 7,
            "voting_open": true,
            "can_vote": true,
            "user_vote": "up",
            "comments": [],
            "comment_count": 0,
            "comments_open": true,
            "can_comment": true,
            "comment_pager": {
                "total_pages": 1,
                "current_page": 1,
                "has_prev": false,
                "has_next": false,
                "prev_page": 1,
                "next_page": 1
            }
        }),
    );

    assert!(html.contains("Requested research desk"));
    assert!(html.contains("Edition Desk"));
    assert!(html.contains("Comments"));
}
