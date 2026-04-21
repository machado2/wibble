use axum::response::Html;

use crate::error::Error;
use crate::llm::prompt_registry::SupportedTranslationLanguage;
use crate::wibble_request::WibbleRequest;

mod comments;
mod page;
mod policy;
mod query;
mod render;

pub use comments::{normalize_comment_body, normalize_comments_page};
pub use policy::{article_accepts_public_interactions, can_view_article};
pub use query::{find_article_by_slug, require_article_by_slug};

#[cfg(test)]
use self::page::{
    article_language_href, build_article_language_options, parse_article_research_metadata,
};

#[allow(async_fn_in_trait)]
pub trait GetContent {
    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
        requested_language: Option<SupportedTranslationLanguage>,
    ) -> Result<Html<String>, Error>;
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error>;
}

impl GetContent for WibbleRequest {
    async fn get_content_paged(
        &self,
        slug: &str,
        after_id: Option<String>,
    ) -> Result<Html<String>, Error> {
        let _c = query::find_article_after_id(&self.state.db, slug, after_id).await?;
        Ok(Html("".to_string()))
    }

    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
        requested_language: Option<SupportedTranslationLanguage>,
    ) -> Result<Html<String>, Error> {
        page::render_content_page(self, slug, source, comments_page, requested_language).await
    }
}

#[cfg(test)]
mod tests {
    use axum::response::Html;
    use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

    use crate::entities::{
        article_job, content, prelude::ArticleJob, prelude::Content, prelude::TranslationJob,
    };
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::rate_limit::RequesterTier;
    use crate::services::article_jobs::{
        ArticleJobService, ARTICLE_JOB_PHASE_COMPLETED, ARTICLE_JOB_STATUS_COMPLETED,
    };
    use crate::services::article_language::resolve_article_language;
    use crate::services::site_text::{default_site_language, site_text};
    use crate::test_support::{preferred_language, TestContext};
    use crate::wibble_request::WibbleRequest;

    use super::{
        article_language_href, build_article_language_options, parse_article_research_metadata,
        GetContent,
    };

    fn sample_article(id: &str, slug: &str, title: &str, generating: bool) -> content::ActiveModel {
        content::ActiveModel {
            id: ActiveValue::set(id.to_string()),
            slug: ActiveValue::set(slug.to_string()),
            content: ActiveValue::set(None),
            created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            generating: ActiveValue::set(generating),
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
            title: ActiveValue::set(title.to_string()),
            user_input: ActiveValue::set("Briefing request".to_string()),
            image_prompt: ActiveValue::set(None),
            user_email: ActiveValue::set(None),
            votes: ActiveValue::set(7),
            hot_score: ActiveValue::set(0.0),
            generation_time_ms: ActiveValue::set(None),
            flarum_id: ActiveValue::set(None),
            markdown: ActiveValue::set(Some(
                "## Committee Response\n\nThe standing committee accepted the memo without visible alarm."
                    .to_string(),
            )),
            converted: ActiveValue::set(true),
            longview_count: ActiveValue::set(0),
            impression_count: ActiveValue::set(0),
            click_count: ActiveValue::set(0),
            author_email: ActiveValue::set(None),
            published: ActiveValue::set(true),
            recovered_from_dead_link: ActiveValue::set(false),
        }
    }

    fn sample_request(state: crate::app_state::AppState, request_path: &str) -> WibbleRequest {
        WibbleRequest {
            state,
            style: "style".to_string(),
            request_path: request_path.to_string(),
            auth_user: None,
            requester_tier: RequesterTier::Anonymous,
            rate_limit_key: "anon:test".to_string(),
            site_language: default_site_language(),
            browser_translation_language: None,
            saved_article_language: None,
        }
    }

    #[test]
    fn article_language_href_uses_query_param_when_override_exists() {
        assert_eq!(
            article_language_href(
                find_supported_translation_language("en").unwrap(),
                "story-slug",
                Some("pt"),
            ),
            "/en/content/story-slug?lang=pt"
        );
    }

    #[test]
    fn automatic_language_option_is_active_without_explicit_override() {
        let selection = resolve_article_language(
            None,
            None,
            None,
            find_supported_translation_language("pt"),
            &[],
        );

        let options = build_article_language_options(
            "story-slug",
            site_text(default_site_language()),
            default_site_language(),
            selection,
            find_supported_translation_language("pt"),
        );

        assert!(options[0].active);
        assert_eq!(options[0].label, "Automatic");
    }

    #[test]
    fn requested_language_option_stays_active_while_falling_back() {
        let selection = resolve_article_language(
            find_supported_translation_language("pt"),
            None,
            None,
            None,
            &[],
        );

        let options = build_article_language_options(
            "story-slug",
            site_text(default_site_language()),
            default_site_language(),
            selection,
            None,
        );
        let portuguese = options
            .iter()
            .find(|option| option.label == "Portuguese")
            .unwrap();

        assert!(portuguese.active);
        assert!(portuguese.note.contains("showing English for now"));
    }

    #[test]
    fn saved_language_option_is_marked_active_from_cookie_preference() {
        let selection = resolve_article_language(
            None,
            None,
            find_supported_translation_language("pt"),
            find_supported_translation_language("fr"),
            &[],
        );

        let options = build_article_language_options(
            "story-slug",
            site_text(default_site_language()),
            default_site_language(),
            selection,
            find_supported_translation_language("fr"),
        );
        let portuguese = options
            .iter()
            .find(|option| option.label == "Portuguese")
            .unwrap();

        assert!(portuguese.active);
        assert!(portuguese.note.contains("showing English for now"));
    }

    #[test]
    fn saved_language_option_notes_saved_preference_when_translation_is_available() {
        let selection = resolve_article_language(
            None,
            None,
            find_supported_translation_language("pt"),
            find_supported_translation_language("fr"),
            &[find_supported_translation_language("pt").unwrap()],
        );

        let options = build_article_language_options(
            "story-slug",
            site_text(default_site_language()),
            default_site_language(),
            selection,
            find_supported_translation_language("fr"),
        );
        let portuguese = options
            .iter()
            .find(|option| option.label == "Portuguese")
            .unwrap();

        assert!(portuguese.active);
        assert_eq!(portuguese.note, "Saved for this article");
    }

    #[test]
    fn research_metadata_parses_manual_mode_and_source_count() {
        let metadata = parse_article_research_metadata(
            site_text(default_site_language()),
            Some(r#"{"research":{"mode":"manual","source_count":3}}"#),
        )
        .unwrap();

        assert_eq!(metadata.mode_label, "Requested research desk");
        assert_eq!(metadata.source_count, 3);
    }

    #[test]
    fn research_metadata_ignores_missing_sources() {
        assert!(parse_article_research_metadata(
            site_text(default_site_language()),
            Some(r#"{"research":{"mode":"auto","source_count":0}}"#,)
        )
        .is_none());
    }

    #[tokio::test]
    async fn get_content_renders_research_metadata_and_translation_fallback() {
        let ctx = TestContext::new().await;
        sample_article("story-1", "research-bulletin", "Research Bulletin", false)
            .insert(&ctx.state.db)
            .await
            .unwrap();
        ArticleJob::insert(article_job::ActiveModel {
            id: ActiveValue::set("job-story-1".to_string()),
            article_id: ActiveValue::set(Some("story-1".to_string())),
            requester_key: ActiveValue::set("anon:test".to_string()),
            requester_tier: ActiveValue::set("ANONYMOUS".to_string()),
            author_email: ActiveValue::set(None),
            prompt: ActiveValue::set("Research prompt".to_string()),
            feature_type: ActiveValue::set("create_research_manual".to_string()),
            phase: ActiveValue::set(ARTICLE_JOB_PHASE_COMPLETED.to_string()),
            status: ActiveValue::set(ARTICLE_JOB_STATUS_COMPLETED.to_string()),
            usage_counters: ActiveValue::set(None),
            preview_payload: ActiveValue::set(Some(
                r#"{"research":{"mode":"manual","source_count":3}}"#.to_string(),
            )),
            error_summary: ActiveValue::set(None),
            fail_count: ActiveValue::set(0),
            created_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::set(chrono::Utc::now().naive_utc()),
            started_at: ActiveValue::set(None),
            finished_at: ActiveValue::set(None),
        })
        .exec(&ctx.state.db)
        .await
        .unwrap();

        let job = ArticleJobService::new(ctx.state.clone())
            .finalize_job_state_for_article("story-1")
            .await
            .unwrap()
            .expect("article job should load");
        assert_eq!(
            job.preview_payload.as_deref(),
            Some(r#"{"research":{"mode":"manual","source_count":3}}"#)
        );

        let Html(html) = sample_request(ctx.state.clone(), "/en/content/research-bulletin")
            .get_content(
                "research-bulletin",
                None,
                None,
                Some(preferred_language("pt")),
            )
            .await
            .unwrap();

        assert!(html.contains("Requested research desk"), "{}", html);
        assert!(html.contains("public-source briefs"));
        assert!(html.contains("Portuguese was requested"));
        assert!(TranslationJob::find_by_id("story-1:pt".to_string())
            .one(&ctx.state.db)
            .await
            .unwrap()
            .is_some());
    }

    #[tokio::test]
    async fn stale_generating_row_with_markdown_serves_content_and_clears_flag() {
        let ctx = TestContext::new().await;
        sample_article("story-2", "stale-bulletin", "Stale Bulletin", true)
            .insert(&ctx.state.db)
            .await
            .unwrap();

        let Html(html) = sample_request(ctx.state.clone(), "/en/content/stale-bulletin")
            .get_content("stale-bulletin", None, None, None)
            .await
            .unwrap();
        let article = Content::find_by_id("story-2".to_string())
            .one(&ctx.state.db)
            .await
            .unwrap()
            .unwrap();

        assert!(html.contains("Stale Bulletin"));
        assert!(!html.contains("Waiting for clarification"));
        assert!(
            !article.generating,
            "expected generating flag to clear after serving content"
        );
    }
}
