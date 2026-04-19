use axum::response::Html;
use serde::Serialize;

use crate::error::Error;
use crate::rate_limit::{RateLimitCapability, RateLimitState, RequesterTier};
use crate::wibble_request::WibbleRequest;

use super::MAX_PROMPT_CHARS;

#[derive(Serialize)]
struct PromptPreset {
    label: &'static str,
    prompt: &'static str,
}

fn create_prompt_presets() -> [PromptPreset; 4] {
    [
        PromptPreset {
            label: "Policy Memo",
            prompt: "A national transport ministry begins issuing emotional readiness bulletins alongside delay notices, and employers start asking staff to attach them to leave requests.",
        },
        PromptPreset {
            label: "Civic Desk",
            prompt: "A borough council opens a formal inquiry after one unusually competent pigeon is repeatedly observed directing pedestrian traffic more effectively than the current signage.",
        },
        PromptPreset {
            label: "Sports Tribunal",
            prompt: "A football federation releases a compliance review after every post-match interview starts sounding like a quarterly earnings call and supporters begin demanding clearer guidance.",
        },
        PromptPreset {
            label: "Research Brief",
            prompt: "A respected institute publishes a sober report concluding that the national mood is best classified as 'manageable, with nuggets', prompting immediate parliamentary interest.",
        },
    ]
}

pub async fn render_create_page(
    wr: &WibbleRequest,
    prompt: &str,
    error_message: Option<&str>,
) -> Result<Html<String>, Error> {
    let presets = create_prompt_presets();
    let logged_in = wr.auth_user.is_some();
    let standard_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::PlainArticleGeneration,
        wr.requester_tier,
    );
    let authenticated_standard_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::PlainArticleGeneration,
        RequesterTier::Authenticated,
    );
    let authenticated_edit_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::EditAgentRequest,
        RequesterTier::Authenticated,
    );
    let authenticated_translation_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::BackgroundTranslation,
        RequesterTier::Authenticated,
    );
    let mut template = wr.template("create").await;
    template
        .insert("title", "Create a new article")
        .insert(
            "description",
            "Submit a brief and let The Wibble draft a straight-faced satirical report.",
        )
        .insert("robots", "noindex,nofollow")
        .insert("prompt", &prompt)
        .insert("prompt_max_length", &MAX_PROMPT_CHARS)
        .insert("prompt_presets", &presets)
        .insert("logged_in", &logged_in)
        .insert("standard_quota", &standard_quota)
        .insert(
            "authenticated_standard_quota",
            &authenticated_standard_quota,
        )
        .insert("authenticated_edit_quota", &authenticated_edit_quota)
        .insert(
            "authenticated_translation_quota",
            &authenticated_translation_quota,
        );
    if let Some(error_message) = error_message {
        template.insert("error_message", error_message);
    }
    template.render()
}

pub async fn get_create(wr: WibbleRequest) -> Result<Html<String>, Error> {
    render_create_page(&wr, "", None).await
}
