use sea_orm::EntityTrait;
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::entities::content;
use crate::entities::prelude::*;
use crate::error::Error;
use crate::services::article_jobs::{ArticleJobRequest, ArticleJobService, ArticleJobTrace};

use super::create_article;

fn recover_prompt_from_slug(slug: &str) -> String {
    let topic = slug
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    if topic.is_empty() {
        slug.to_string()
    } else {
        topic
    }
}

pub async fn start_recover_article_for_slug(
    state: AppState,
    slug: String,
) -> Result<Option<String>, Error> {
    let job_service = ArticleJobService::new(state.clone());
    let slug = slug.trim().to_string();
    if slug.is_empty() {
        return Ok(None);
    }

    let permit = job_service.try_acquire_generation_slot("dead_link_recovery")?;

    let model = state
        .llm
        .models
        .first()
        .ok_or_else(|| Error::Llm("No language model configured".to_string()))?
        .to_string();

    let id = job_service.new_job_id();
    let return_id = id.clone();
    let prompt = recover_prompt_from_slug(&slug);
    let now = chrono::Utc::now().naive_local();
    let placeholder = content::Model {
        id: id.clone(),
        slug: slug.clone(),
        content: None,
        created_at: now,
        generating: true,
        generation_started_at: Some(now),
        generation_finished_at: None,
        flagged: false,
        model: model.clone(),
        prompt_version: 0,
        fail_count: 0,
        description: format!("Recovered dead link: /content/{}", slug),
        image_id: None,
        title: slug.replace('-', " "),
        user_input: prompt.clone(),
        image_prompt: None,
        user_email: None,
        votes: 0,
        hot_score: 0.0,
        generation_time_ms: None,
        flarum_id: None,
        markdown: None,
        converted: false,
        longview_count: 0,
        impression_count: 0,
        click_count: 0,
        author_email: None,
        published: true,
        recovered_from_dead_link: true,
    };
    let insert_result = Content::insert(content::ActiveModel::from(placeholder))
        .exec(&state.db)
        .await;
    if insert_result.is_err() {
        event!(
            Level::WARN,
            slug = %slug,
            "Dead-link recovery placeholder insert failed; likely slug already exists"
        );
        return Ok(None);
    }

    if !state.try_take_dead_link_recovery_slot().await {
        event!(
            Level::WARN,
            slug = %slug,
            max_per_day = state.dead_link_recovery_max_per_day,
            "Dead-link recovery skipped due to daily limit"
        );
        let _ = Content::delete_by_id(id).exec(&state.db).await;
        return Ok(None);
    }

    if let Err(err) = job_service
        .create_job(
            id.clone(),
            ArticleJobRequest::dead_link_recovery(prompt.clone(), id.clone()),
        )
        .await
    {
        let _ = Content::delete_by_id(id.clone()).exec(&state.db).await;
        return Err(err);
    }

    job_service
        .spawn_generation_job(
            id.clone(),
            permit,
            ArticleJobTrace::dead_link_recovery(slug.clone()),
            async move {
                let result = create_article(&state, id.clone(), prompt, None, None).await;
                if result.is_err() {
                    let _ = Content::delete_by_id(id.clone()).exec(&state.db).await;
                }
                result
            },
        )
        .await;

    Ok(Some(return_id))
}

#[cfg(test)]
mod tests {
    use super::recover_prompt_from_slug;

    #[test]
    fn recover_prompt_turns_slug_into_space_separated_topic() {
        assert_eq!(
            recover_prompt_from_slug("cabinet-shuffle_under-pressure"),
            "cabinet shuffle under pressure"
        );
    }

    #[test]
    fn recover_prompt_falls_back_to_original_slug_when_topic_is_empty() {
        assert_eq!(recover_prompt_from_slug("---"), "---");
    }
}
