mod definitions;
mod queue;
mod refresh;
mod support;
mod worker;

pub use self::definitions::{
    request_source_from_preferred_language, TranslationJobRequestSource,
    TRANSLATION_JOB_STATUS_CANCELLED, TRANSLATION_JOB_STATUS_COMPLETED,
    TRANSLATION_JOB_STATUS_FAILED, TRANSLATION_JOB_STATUS_PROCESSING,
    TRANSLATION_JOB_STATUS_QUEUED,
};
pub use self::refresh::refresh_article_translations_after_edit;
pub use self::worker::{cancel_translation_job, request_article_translation, spawn_resume_loop};

#[cfg(test)]
use self::queue::{
    due_translation_jobs, load_translation_job, persist_translation_job_request,
    stale_translation_languages_for_refresh,
};
#[cfg(test)]
use self::support::translation_retry_delay;
#[cfg(test)]
use self::worker::process_translation_job_with_translator;

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveModelTrait, ActiveValue, EntityTrait};

    use crate::entities::{content, prelude::TranslationJob, translation_job};
    use crate::error::Error;
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::llm::translate::Translate;
    use crate::rate_limit::RequesterTier;
    use crate::services::article_language::PreferredLanguageSource;
    use crate::services::article_translations::{
        load_cached_article_translation, ArticleSourceText,
    };
    use crate::test_support::{preferred_language, test_state_for, TestContext};

    use super::{
        due_translation_jobs, load_translation_job, persist_translation_job_request,
        process_translation_job_with_translator, request_source_from_preferred_language,
        stale_translation_languages_for_refresh, translation_retry_delay,
        TranslationJobRequestSource, TRANSLATION_JOB_STATUS_COMPLETED,
        TRANSLATION_JOB_STATUS_PROCESSING,
    };

    #[derive(Default)]
    struct FakeTranslator;

    impl Translate for FakeTranslator {
        async fn translate(
            &self,
            text: &str,
            target_language: crate::llm::prompt_registry::SupportedTranslationLanguage,
        ) -> Result<String, Error> {
            Ok(format!("[{}] {}", target_language.code, text))
        }
    }

    fn sample_article(id: &str, slug: &str) -> content::ActiveModel {
        content::ActiveModel {
            id: ActiveValue::set(id.to_string()),
            slug: ActiveValue::set(slug.to_string()),
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

    fn article_source<'a>(article_id: &'a str) -> ArticleSourceText<'a> {
        ArticleSourceText {
            article_id,
            title: "Research Bulletin",
            description: "Officials said the bulletin remained strictly procedural.",
            markdown: "## Committee Response\n\nThe standing committee accepted the memo without visible alarm.",
        }
    }

    #[test]
    fn request_source_preserves_manual_priority_over_browser_defaults() {
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Explicit),
            TranslationJobRequestSource::Explicit
        );
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Cookie),
            TranslationJobRequestSource::Cookie
        );
        assert_eq!(
            request_source_from_preferred_language(PreferredLanguageSource::Browser),
            TranslationJobRequestSource::Browser
        );
    }

    #[test]
    fn translation_retry_delay_grows_with_failures_and_caps() {
        let first = translation_retry_delay(1);
        let second = translation_retry_delay(2);
        let sixth = translation_retry_delay(6);

        assert!(second > first);
        assert!(sixth >= second);
        assert!(sixth.as_secs() <= 15 * 60);
    }

    #[test]
    fn stale_translation_refresh_excludes_source_language() {
        let languages = vec![
            find_supported_translation_language("en").unwrap(),
            find_supported_translation_language("pt").unwrap(),
            find_supported_translation_language("fr").unwrap(),
        ];

        let stale = stale_translation_languages_for_refresh(&languages);

        assert_eq!(stale.len(), 2);
        assert!(stale.iter().all(|language| language.code != "en"));
    }

    #[tokio::test]
    async fn queued_translation_job_generates_cache_and_marks_completed() {
        let ctx = TestContext::new().await;
        sample_article("story-1", "research-bulletin")
            .insert(&ctx.state.db)
            .await
            .unwrap();
        persist_translation_job_request(
            &ctx.state,
            "story-1",
            preferred_language("pt"),
            TranslationJobRequestSource::Explicit,
            RequesterTier::Authenticated,
            "user:author@example.com",
            false,
        )
        .await
        .unwrap();

        process_translation_job_with_translator(&ctx.state, &FakeTranslator, "story-1:pt")
            .await
            .unwrap();

        let translation = load_cached_article_translation(
            &ctx.state.db,
            article_source("story-1"),
            preferred_language("pt"),
        )
        .await
        .unwrap()
        .expect("translation should be cached");
        let job = load_translation_job(&ctx.state.db, "story-1:pt")
            .await
            .unwrap()
            .expect("translation job should exist");

        assert!(translation.title.starts_with("[pt]"));
        assert_eq!(job.status, TRANSLATION_JOB_STATUS_COMPLETED);
    }

    #[tokio::test]
    async fn processing_translation_job_is_resumed_after_restart() {
        let ctx = TestContext::new().await;
        sample_article("story-2", "restart-bulletin")
            .insert(&ctx.state.db)
            .await
            .unwrap();
        persist_translation_job_request(
            &ctx.state,
            "story-2",
            preferred_language("fr"),
            TranslationJobRequestSource::Explicit,
            RequesterTier::Authenticated,
            "user:author@example.com",
            false,
        )
        .await
        .unwrap();
        let mut active: translation_job::ActiveModel =
            load_translation_job(&ctx.state.db, "story-2:fr")
                .await
                .unwrap()
                .expect("translation job should exist")
                .into();
        active.status = ActiveValue::set(TRANSLATION_JOB_STATUS_PROCESSING.to_string());
        active.started_at = ActiveValue::set(Some(chrono::Utc::now().naive_utc()));
        active.update(&ctx.state.db).await.unwrap();

        let restarted_state = test_state_for(&ctx.db.url).await;
        let due = due_translation_jobs(&restarted_state).await.unwrap();
        assert!(due.iter().any(|job| job.id == "story-2:fr"));

        process_translation_job_with_translator(&restarted_state, &FakeTranslator, "story-2:fr")
            .await
            .unwrap();

        let translation = load_cached_article_translation(
            &restarted_state.db,
            article_source("story-2"),
            preferred_language("fr"),
        )
        .await
        .unwrap()
        .expect("translation should be cached after restart");
        let resumed_job = TranslationJob::find_by_id("story-2:fr".to_string())
            .one(&restarted_state.db)
            .await
            .unwrap()
            .expect("translation job should remain persisted");

        assert!(translation.markdown.starts_with("[fr]"));
        assert_eq!(resumed_job.status, TRANSLATION_JOB_STATUS_COMPLETED);
    }
}
