use axum::response::Html;
use serde::Serialize;

use crate::error::Error;
use crate::rate_limit::{RateLimitCapability, RateLimitState, RequesterTier};
use crate::wibble_request::WibbleRequest;

use super::{CreateModeSelection, MAX_PROMPT_CHARS};

#[derive(Serialize)]
struct PromptPreset {
    label: &'static str,
    prompt: &'static str,
}

pub async fn render_create_page(
    wr: &WibbleRequest,
    prompt: &str,
    error_message: Option<&str>,
    selected_mode: CreateModeSelection,
) -> Result<Html<String>, Error> {
    let text = wr.site_text();
    let ui = text.template_strings();
    let presets = text
        .create_prompt_presets()
        .map(|(label, prompt)| PromptPreset { label, prompt });
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
    let research_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::ResearchGeneration,
        wr.requester_tier,
    );
    let authenticated_research_quota = RateLimitState::quota_summary_for(
        RateLimitCapability::ResearchGeneration,
        RequesterTier::Authenticated,
    );
    let mut template = wr.template("create").await;
    let owner_editing_note = ui["create"]["owner_editing_note"]
        .as_str()
        .unwrap_or_default()
        .replace(
            "%EDIT_HOURLY%",
            &authenticated_edit_quota.hourly.to_string(),
        );
    let research_lane_note = ui["create"]["research_lane_note"]
        .as_str()
        .unwrap_or_default()
        .replace("%RESEARCH_HOURLY%", &research_quota.hourly.to_string())
        .replace("%RESEARCH_DAILY%", &research_quota.daily.to_string());
    let translation_lane_note = ui["create"]["translation_lane_note"]
        .as_str()
        .unwrap_or_default()
        .replace(
            "%TRANSLATION_HOURLY%",
            &authenticated_translation_quota.hourly.to_string(),
        );
    let login_upsell_note = ui["create"]["login_upsell"]
        .as_str()
        .unwrap_or_default()
        .replace(
            "%STANDARD_HOURLY%",
            &authenticated_standard_quota.hourly.to_string(),
        )
        .replace(
            "%RESEARCH_HOURLY%",
            &authenticated_research_quota.hourly.to_string(),
        );
    let research_quota_note = ui["create"]["mode_research_quota"]
        .as_str()
        .unwrap_or_default()
        .replace("%RESEARCH_HOURLY%", &research_quota.hourly.to_string())
        .replace("%RESEARCH_DAILY%", &research_quota.daily.to_string());
    template
        .insert("title", text.create_meta_title())
        .insert("description", text.create_meta_description())
        .insert("robots", "noindex,nofollow")
        .insert("prompt", &prompt)
        .insert("prompt_max_length", &MAX_PROMPT_CHARS)
        .insert("prompt_presets", &presets)
        .insert("logged_in", &logged_in)
        .insert("selected_create_mode", selected_mode.as_str())
        .insert("owner_editing_note", &owner_editing_note)
        .insert("research_lane_note", &research_lane_note)
        .insert("translation_lane_note", &translation_lane_note)
        .insert("login_upsell_note", &login_upsell_note)
        .insert("research_quota_note", &research_quota_note)
        .insert("standard_quota", &standard_quota)
        .insert("research_quota", &research_quota)
        .insert(
            "authenticated_standard_quota",
            &authenticated_standard_quota,
        )
        .insert(
            "authenticated_research_quota",
            &authenticated_research_quota,
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
    render_create_page(&wr, "", None, CreateModeSelection::Auto).await
}
