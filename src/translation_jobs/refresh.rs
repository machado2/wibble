use crate::app_state::AppState;
use crate::audit::log_audit;
use crate::auth::AuthUser;
use crate::error::Error;
use crate::rate_limit::RequesterTier;
use crate::services::article_translations::{
    cached_translation_languages, invalidate_cached_article_translations, OwnedArticleSourceText,
};

use super::queue::{queue_translation_refresh, stale_translation_languages_for_refresh};
use super::worker::spawn_due_translation_jobs;

pub async fn refresh_article_translations_after_edit(
    state: AppState,
    auth_user: &AuthUser,
    slug: &str,
    previous_source: OwnedArticleSourceText,
    current_source: OwnedArticleSourceText,
) -> Result<(), Error> {
    if previous_source == current_source {
        return Ok(());
    }

    let cached_languages =
        cached_translation_languages(&state.db, previous_source.as_ref()).await?;
    if cached_languages.is_empty() {
        return Ok(());
    }

    let stale_languages = stale_translation_languages_for_refresh(&cached_languages);
    if stale_languages.is_empty() {
        return Ok(());
    }

    let removed_rows =
        invalidate_cached_article_translations(&state.db, previous_source.as_ref()).await?;
    let details = serde_json::json!({
        "languages": stale_languages
            .iter()
            .map(|language| language.code)
            .collect::<Vec<_>>(),
        "removed_rows": removed_rows,
    })
    .to_string();
    log_audit(
        &state.db,
        auth_user,
        "invalidate_article_translations",
        "content",
        slug,
        Some(details),
    )
    .await?;

    for language in stale_languages {
        let requester_tier = if auth_user.is_admin() {
            RequesterTier::Admin
        } else {
            RequesterTier::Authenticated
        };
        let rate_limit_key = format!("user:{}", auth_user.email);
        queue_translation_refresh(
            &state,
            &current_source.article_id,
            language,
            requester_tier,
            &rate_limit_key,
        )
        .await?;
    }
    spawn_due_translation_jobs(state).await;
    Ok(())
}
