use serde_json::{json, Value};

use crate::services::article_jobs::{
    ARTICLE_JOB_PHASE_AWAITING_USER_INPUT, ARTICLE_JOB_PHASE_QUEUED,
    ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RENDERING_IMAGES,
    ARTICLE_JOB_PHASE_RESEARCHING,
};

#[derive(Clone, Copy, Debug)]
pub struct SiteText;

pub fn site_text() -> SiteText {
    SiteText
}

impl SiteText {
    pub fn template_strings(self) -> Value {
        json!({
            "edit": {
                "accepted_formats": "Accepted formats: JPG, JPEG, PNG. Maximum size: 12 MB.", "agent_edit": "Agent edit", "agent_edit_body": "Describe the revision you want.",
                "cancel": "Cancel", "change_request": "Change request", "change_request_placeholder": "Describe the change...", "content_label": "Article body",
                "description_label": "Description", "eyebrow": "Editor", "images_body": "Replace or regenerate article images.", "images_title": "Images",
                "jump_to_images": "Jump to images", "last_error_prefix": "Last error:", "manual_editor": "Manual editor", "owners_only": "Administrators only.",
                "preview_revision": "Preview revision", "raw_markdown_note": "Markdown", "regenerate_prompt": "Regenerate from prompt", "regenerating": "Regenerating...",
                "replace_image": "Replace image", "save_changes": "Save changes", "status_prefix": "Status:", "title": "Edit article", "title_label": "Title",
                "view_article": "View article", "workspace_note": "Edit the article and save when ready."
            },
            "edit_preview": {
                "agent_summary": "Agent summary", "apply_revision": "Apply agent revision", "back_to_editor": "Back to editor", "current": "Current",
                "description_label": "Description", "diff": "Diff", "discard_preview": "Discard preview", "eyebrow": "Editor", "field_preview": "Field preview",
                "markdown_label": "Markdown", "prompt_version": "Prompt version:", "proposed": "Proposed", "requested_change": "Requested change",
                "title": "Agent edit preview", "title_label": "Title", "view_article": "View article"
            },
            "error": {"image_alt": "Error illustration"},
            "image_info": {"created": "Created", "kicker": "Generated image", "last_error": "Last error", "metadata_aria": "Image metadata", "model": "Model", "status": "Status", "used_in": "Used in"},
            "images": {"empty_body": "No generated images are available.", "empty_title": "No images", "eyebrow": "Archive", "next_page": "Next page", "summary": "Generated images used in Wibble News articles.", "title": "Generated images", "used_in": "Used in"}
        })
    }

    pub fn wait_meta_title(self) -> &'static str {
        "Generating article"
    }
    pub fn wait_meta_description(self) -> &'static str {
        "The article is still being generated and this page auto-refreshes."
    }
    pub fn wait_publication_copy(self, _is_logged_in: bool) -> (String, String) {
        (
            "Destination: public".into(),
            "Articles publish immediately.".into(),
        )
    }
    pub fn wait_stage_copy(self, phase: Option<&str>) -> (String, String) {
        match phase {
            Some(ARTICLE_JOB_PHASE_QUEUED) => (
                "Queued for generation".into(),
                "The prompt is waiting for a generation slot.".into(),
            ),
            Some(ARTICLE_JOB_PHASE_RESEARCHING) => (
                "Researching the brief".into(),
                "The job is gathering bounded context before drafting.".into(),
            ),
            Some(ARTICLE_JOB_PHASE_RENDERING_IMAGES) => (
                "Rendering illustrations".into(),
                "The article is ready and its images are rendering.".into(),
            ),
            Some(ARTICLE_JOB_PHASE_READY_FOR_REVIEW) => (
                "Finalizing the article".into(),
                "The page is about to go live.".into(),
            ),
            Some(ARTICLE_JOB_PHASE_AWAITING_USER_INPUT) => {
                ("Waiting".into(), "The job is paused.".into())
            }
            _ => (
                "Drafting the story".into(),
                "The article is being written.".into(),
            ),
        }
    }
    pub fn wait_image_stage_copy(
        self,
        total: usize,
        complete: usize,
        processing: usize,
        failed: usize,
        markdown_ready: bool,
    ) -> (String, String) {
        if total == 0 && !markdown_ready {
            self.wait_stage_copy(None)
        } else if processing > 0 {
            self.wait_stage_copy(Some(ARTICLE_JOB_PHASE_RENDERING_IMAGES))
        } else if failed > 0 && complete < total {
            (
                "Recovering the image set".into(),
                "Some images failed and the article is waiting for the remaining results.".into(),
            )
        } else {
            self.wait_stage_copy(Some(ARTICLE_JOB_PHASE_READY_FOR_REVIEW))
        }
    }
    pub fn wait_phase_label(self, phase: &str) -> &'static str {
        match phase {
            ARTICLE_JOB_PHASE_QUEUED => "Queued",
            ARTICLE_JOB_PHASE_AWAITING_USER_INPUT => "Clarify",
            ARTICLE_JOB_PHASE_RENDERING_IMAGES => "Images",
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW => "Review",
            _ => "Write",
        }
    }
    pub fn wait_clarification_deadline_note(self, deadline: &str) -> String {
        format!("The job resumes automatically after {}.", deadline)
    }
    pub fn server_error_title(self) -> &'static str {
        "Server error"
    }
    pub fn server_error_description(self) -> &'static str {
        "An unexpected server error occurred while loading this page."
    }
    pub fn server_error_message(self) -> &'static str {
        "Oops! Something went wrong. Please try again later."
    }
    pub fn not_found_title(self) -> &'static str {
        "Page not found"
    }
    pub fn not_found_description(self) -> &'static str {
        "The requested page could not be found."
    }
    pub fn not_found_message(self) -> &'static str {
        "The page you are looking for does not exist."
    }
    pub fn image_gallery_meta_title(self) -> &'static str {
        "Generated image gallery"
    }
    pub fn image_gallery_meta_description(self) -> &'static str {
        "A gallery of generated images used in Wibble stories."
    }
    pub fn image_info_description(self) -> &'static str {
        "Generation details for an image used in a Wibble article."
    }
    pub fn edit_meta_title(self, title: &str) -> String {
        format!("Edit: {}", title)
    }
    pub fn edit_preview_meta_title(self, title: &str) -> String {
        format!("Agent edit preview: {}", title)
    }
    pub fn image_status_label(self, status: &str) -> &'static str {
        match status {
            "pending" => "Queued",
            "processing" => "Generating",
            "failed" => "Failed",
            "completed" => "Ready",
            _ => "Unknown",
        }
    }
    pub fn image_status_note(self, status: &str) -> &'static str {
        match status {
            "pending" => "A fresh render is queued.",
            "processing" => "A fresh render is in progress.",
            "failed" => "The last generation attempt failed.",
            "completed" => "Current stored image.",
            _ => "Image status is available.",
        }
    }
}
