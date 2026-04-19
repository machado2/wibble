use std::sync::atomic::Ordering;

use sea_orm::EntityTrait;
use tracing::{event, Level};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::entities::content;
use crate::entities::prelude::*;
use crate::error::Error;

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
    let slug = slug.trim().to_string();
    if slug.is_empty() {
        return Ok(None);
    }

    let permit = state
        .article_generation_semaphore
        .clone()
        .try_acquire_owned()
        .map_err(|_| {
            event!(
                Level::WARN,
                "Rejected dead-link recovery due to concurrency limit (MAX_CONCURRENT_ARTICLE_GENERATIONS reached)",
            );
            Error::RateLimited
        })?;

    let model = state
        .llm
        .models
        .first()
        .ok_or_else(|| Error::Llm("No language model configured".to_string()))?
        .to_string();

    let id = Uuid::new_v4().to_string();
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

    let active_counter = state.active_article_generations.clone();
    state.mark_generation_started(&id).await;
    state
        .task_list
        .clone()
        .spawn_task(id.clone(), async move {
            let _permit = permit;
            let in_flight = active_counter.fetch_add(1, Ordering::SeqCst) + 1;
            event!(
                Level::INFO,
                article_id = %id,
                recovery_slug = %slug,
                in_flight,
                "Started dead-link recovery generation task"
            );
            let result = create_article(&state, id.clone(), prompt, None).await;
            let in_flight_after = active_counter
                .fetch_sub(1, Ordering::SeqCst)
                .saturating_sub(1);
            if result.is_err() {
                let _ = Content::delete_by_id(id.clone()).exec(&state.db).await;
            }
            event!(
                Level::INFO,
                article_id = %id,
                recovery_slug = %slug,
                in_flight = in_flight_after,
                "Finished dead-link recovery generation task"
            );
            state.mark_generation_finished(&id).await;
            result
        })
        .await;

    Ok(Some(return_id))
}
