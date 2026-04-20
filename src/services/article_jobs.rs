mod definitions;
mod management;
mod resume;
mod runtime;
mod support;

pub use self::definitions::{
    is_in_progress_job_status, is_terminal_job_status, ArticleJobFeatureType, ArticleJobRequest,
    ArticleJobService, ArticleJobTrace, ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
    ARTICLE_JOB_PHASE_CANCELLED, ARTICLE_JOB_PHASE_COMPLETED, ARTICLE_JOB_PHASE_EDITING,
    ARTICLE_JOB_PHASE_FAILED, ARTICLE_JOB_PHASE_PLANNING, ARTICLE_JOB_PHASE_QUEUED,
    ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RENDERING_IMAGES,
    ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_TRANSLATING, ARTICLE_JOB_PHASE_WRITING,
    ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED, ARTICLE_JOB_STATUS_FAILED,
    ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
};
pub use self::resume::{spawn_due_article_jobs, spawn_resume_loop};

#[cfg(test)]
use self::definitions::ImageProgress;
#[cfg(test)]
use self::support::{build_job_preview_payload, default_usage_counters, merge_job_usage_counters};

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveValue, EntityTrait};

    use crate::create::clarify::build_clarification_request;
    use crate::entities::content;
    use crate::entities::{article_job, prelude::ArticleJob};
    use crate::llm::article_generator::ResearchModeSource;
    use crate::rate_limit::RequesterTier;
    use crate::test_support::TestContext;
    use serde_json::Value;

    use super::{
        build_job_preview_payload, default_usage_counters, is_in_progress_job_status,
        is_terminal_job_status, merge_job_usage_counters, ArticleJobFeatureType, ArticleJobRequest,
        ArticleJobService, ImageProgress, ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
        ARTICLE_JOB_PHASE_CANCELLED, ARTICLE_JOB_PHASE_COMPLETED, ARTICLE_JOB_PHASE_EDITING,
        ARTICLE_JOB_PHASE_FAILED, ARTICLE_JOB_PHASE_PLANNING, ARTICLE_JOB_PHASE_QUEUED,
        ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RENDERING_IMAGES,
        ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_TRANSLATING, ARTICLE_JOB_PHASE_WRITING,
        ARTICLE_JOB_STATUS_CANCELLED, ARTICLE_JOB_STATUS_COMPLETED, ARTICLE_JOB_STATUS_FAILED,
        ARTICLE_JOB_STATUS_PROCESSING, ARTICLE_JOB_STATUS_QUEUED,
    };

    fn sample_article() -> content::Model {
        content::Model {
            id: "article-1".to_string(),
            slug: "article-1".to_string(),
            content: None,
            created_at: chrono::NaiveDate::from_ymd_opt(2026, 4, 19)
                .unwrap()
                .and_hms_opt(12, 0, 0)
                .unwrap(),
            generating: false,
            generation_started_at: None,
            generation_finished_at: None,
            flagged: false,
            model: "model".to_string(),
            prompt_version: 1,
            fail_count: 0,
            description: "desc".to_string(),
            image_id: None,
            title: "headline".to_string(),
            user_input: "input".to_string(),
            image_prompt: None,
            user_email: None,
            votes: 0,
            hot_score: 0.0,
            generation_time_ms: None,
            flarum_id: None,
            markdown: Some("body".to_string()),
            converted: false,
            longview_count: 0,
            impression_count: 0,
            click_count: 0,
            author_email: Some("author@example.com".to_string()),
            published: false,
            recovered_from_dead_link: false,
        }
    }

    #[test]
    fn explicit_phase_constants_cover_persisted_agent_lifecycle() {
        let phases = [
            ARTICLE_JOB_PHASE_QUEUED,
            ARTICLE_JOB_PHASE_PLANNING,
            ARTICLE_JOB_PHASE_RESEARCHING,
            ARTICLE_JOB_PHASE_AWAITING_USER_INPUT,
            ARTICLE_JOB_PHASE_WRITING,
            ARTICLE_JOB_PHASE_EDITING,
            ARTICLE_JOB_PHASE_TRANSLATING,
            ARTICLE_JOB_PHASE_RENDERING_IMAGES,
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW,
            ARTICLE_JOB_PHASE_COMPLETED,
            ARTICLE_JOB_PHASE_FAILED,
            ARTICLE_JOB_PHASE_CANCELLED,
        ];

        assert!(phases.contains(&ARTICLE_JOB_PHASE_RENDERING_IMAGES));
        assert!(phases.contains(&ARTICLE_JOB_PHASE_READY_FOR_REVIEW));
    }

    #[test]
    fn job_status_helpers_distinguish_terminal_and_active_states() {
        assert!(is_in_progress_job_status(ARTICLE_JOB_STATUS_QUEUED));
        assert!(is_in_progress_job_status(ARTICLE_JOB_STATUS_PROCESSING));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_COMPLETED));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_FAILED));
        assert!(is_terminal_job_status(ARTICLE_JOB_STATUS_CANCELLED));
        assert!(!is_terminal_job_status(ARTICLE_JOB_STATUS_QUEUED));
    }

    #[test]
    fn usage_counters_include_runtime_and_image_progress() {
        let mut usage = default_usage_counters(14);
        usage
            .as_object_mut()
            .unwrap()
            .insert("model_calls".to_string(), Value::from(2));
        let json = merge_job_usage_counters(
            Some(&usage.to_string()),
            "deadpan prompt",
            &ImageProgress {
                total: 3,
                completed: 1,
                processing: 1,
                failed: 1,
                pending_ids: vec!["img-2".to_string()],
            },
        );

        assert!(json.contains("\"prompt_chars\":14"));
        assert!(json.contains("\"model_calls\":2"));
        assert!(json.contains("\"image_total\":3"));
        assert!(json.contains("\"image_failed\":1"));
    }

    #[test]
    fn research_feature_types_round_trip_mode_source() {
        let auto = ArticleJobFeatureType::from_research_mode(Some(ResearchModeSource::Auto));
        let manual = ArticleJobFeatureType::from_research_mode(Some(ResearchModeSource::Manual));

        assert_eq!(auto.as_str(), "create_research_auto");
        assert_eq!(manual.as_str(), "create_research_manual");
        assert_eq!(
            ArticleJobFeatureType::from_str(auto.as_str())
                .unwrap()
                .research_mode(),
            Some(ResearchModeSource::Auto)
        );
        assert_eq!(
            ArticleJobFeatureType::from_str(manual.as_str())
                .unwrap()
                .research_mode(),
            Some(ResearchModeSource::Manual)
        );
    }

    #[test]
    fn preview_payload_merge_preserves_existing_research_metadata() {
        let payload = build_job_preview_payload(
            Some(r#"{"research":{"mode":"manual","source_count":2}}"#),
            &sample_article(),
            &ImageProgress {
                total: 2,
                completed: 1,
                processing: 0,
                failed: 1,
                pending_ids: Vec::new(),
            },
        );

        assert!(payload.contains("\"research\":{\"mode\":\"manual\",\"source_count\":2}"));
        assert!(payload.contains("\"publication_state\":\"draft\""));
        assert!(payload.contains("\"image_failed\":1"));
    }

    #[tokio::test]
    async fn clarification_request_and_answer_resume_job_from_persisted_state() {
        let ctx = TestContext::new().await;
        let service = ArticleJobService::new(ctx.state.clone());
        let job_id = "job-clarify".to_string();
        let prompt = "A local office begins doing something odd".to_string();
        service
            .create_job(
                job_id.clone(),
                ArticleJobRequest::create(
                    prompt.clone(),
                    Some("author@example.com".to_string()),
                    RequesterTier::Authenticated,
                    "user:author@example.com".to_string(),
                    None,
                ),
            )
            .await
            .unwrap();
        let clarification = build_clarification_request(&prompt).expect("clarification expected");

        service
            .request_clarification(&job_id, serde_json::to_string(&clarification).unwrap())
            .await
            .unwrap();
        let waiting = ArticleJob::find_by_id(job_id.clone())
            .one(&ctx.state.db)
            .await
            .unwrap()
            .expect("article job should exist");
        assert_eq!(waiting.phase, ARTICLE_JOB_PHASE_AWAITING_USER_INPUT);
        assert_eq!(waiting.status, ARTICLE_JOB_STATUS_PROCESSING);

        let resumed = service
            .submit_clarification_answer(&job_id, "The transport ministry")
            .await
            .unwrap()
            .expect("job should resume");

        assert_eq!(resumed.phase, ARTICLE_JOB_PHASE_QUEUED);
        assert_eq!(resumed.status, ARTICLE_JOB_STATUS_QUEUED);
        assert!(resumed.prompt.contains("Clarification from requester"));
        assert!(resumed.prompt.contains("The transport ministry"));
        assert!(resumed.preview_payload.is_none());
    }

    #[tokio::test]
    async fn research_job_preview_payload_round_trips_through_persisted_job_row() {
        let ctx = TestContext::new().await;
        let now = chrono::Utc::now().naive_utc();
        ArticleJob::insert(article_job::ActiveModel {
            id: ActiveValue::set("job-research".to_string()),
            article_id: ActiveValue::set(Some("article-1".to_string())),
            requester_key: ActiveValue::set("user:author@example.com".to_string()),
            requester_tier: ActiveValue::set("AUTHENTICATED".to_string()),
            author_email: ActiveValue::set(Some("author@example.com".to_string())),
            prompt: ActiveValue::set("Research prompt".to_string()),
            feature_type: ActiveValue::set("create_research_manual".to_string()),
            phase: ActiveValue::set(ARTICLE_JOB_PHASE_COMPLETED.to_string()),
            status: ActiveValue::set(ARTICLE_JOB_STATUS_COMPLETED.to_string()),
            usage_counters: ActiveValue::set(None),
            preview_payload: ActiveValue::set(Some(
                r#"{"research":{"mode":"manual","source_count":2}}"#.to_string(),
            )),
            error_summary: ActiveValue::set(None),
            fail_count: ActiveValue::set(0),
            created_at: ActiveValue::set(now),
            updated_at: ActiveValue::set(now),
            started_at: ActiveValue::set(None),
            finished_at: ActiveValue::set(Some(now)),
        })
        .exec(&ctx.state.db)
        .await
        .unwrap();

        let loaded = ArticleJobService::new(ctx.state.clone())
            .finalize_job_state_for_article("article-1")
            .await
            .unwrap()
            .expect("article job should load");

        assert_eq!(
            loaded.preview_payload.as_deref(),
            Some(r#"{"research":{"mode":"manual","source_count":2}}"#)
        );
        assert_eq!(loaded.feature_type, "create_research_manual");
    }
}
