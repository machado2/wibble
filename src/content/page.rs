use axum::response::Html;
use serde::Serialize;
use serde_json::Value;

use crate::error::Error;
use crate::llm::prompt_registry::{supported_translation_languages, SupportedTranslationLanguage};
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::services::article_jobs::ArticleJobService;
use crate::services::article_language::{
    resolve_article_language, ArticleLanguageSelection, PreferredLanguageSource,
    ServedLanguageSource,
};
use crate::services::article_translations::{
    cached_translation_languages, load_cached_article_translation, ArticleSourceText,
};
use crate::translation_jobs::{
    request_article_translation, request_source_from_preferred_language,
};
use crate::wibble_request::WibbleRequest;

use super::{article_accepts_public_interactions, comments, policy, query, render};

#[derive(Serialize)]
pub(super) struct ArticleLanguageOption {
    pub(super) href: String,
    pub(super) label: String,
    pub(super) note: String,
    pub(super) active: bool,
}

#[derive(Serialize)]
pub(super) struct ArticleResearchMetadata {
    pub(super) mode_label: String,
    pub(super) source_count: usize,
}

pub(super) fn article_language_href(slug: &str, lang: Option<&str>) -> String {
    let mut path = format!("/content/{}", slug);
    if let Some(lang) = lang {
        path.push_str("?lang=");
        path.push_str(lang);
    }
    path
}

pub(super) fn build_article_language_options(
    slug: &str,
    selection: ArticleLanguageSelection,
    browser_language: Option<SupportedTranslationLanguage>,
) -> Vec<ArticleLanguageOption> {
    let automatic_note = browser_language
        .map(|language| format!("Browser default: {}", language.name))
        .unwrap_or_else(|| format!("Original edition: {}", selection.source_language.name));
    let mut options = vec![
        ArticleLanguageOption {
            href: article_language_href(slug, Some("auto")),
            label: "Automatic".to_string(),
            note: automatic_note,
            active: !uses_manual_article_language_preference(selection.preferred_language_source),
        },
        ArticleLanguageOption {
            href: article_language_href(slug, Some(selection.source_language.code)),
            label: format!("Original ({})", selection.source_language.name),
            note: "Manual source edition".to_string(),
            active: uses_manual_article_language_preference(selection.preferred_language_source)
                && selection.preferred_language.code == selection.source_language.code,
        },
    ];

    options.extend(
        supported_translation_languages()
            .iter()
            .copied()
            .filter(|language| language.code != selection.source_language.code)
            .map(|language| {
                let manually_selected =
                    uses_manual_article_language_preference(selection.preferred_language_source)
                        && selection.preferred_language.code == language.code;
                let note = if manually_selected && !selection.translation_available {
                    format!(
                        "Requested; showing {} for now",
                        selection.served_language.name
                    )
                } else if manually_selected
                    && selection.preferred_language_source == PreferredLanguageSource::Cookie
                {
                    "Saved for this article".to_string()
                } else if manually_selected {
                    "Selected edition".to_string()
                } else {
                    "Open when available".to_string()
                };

                ArticleLanguageOption {
                    href: article_language_href(slug, Some(language.code)),
                    label: language.name.to_string(),
                    note,
                    active: manually_selected,
                }
            }),
    );

    options
}

pub(super) fn uses_manual_article_language_preference(source: PreferredLanguageSource) -> bool {
    matches!(
        source,
        PreferredLanguageSource::Explicit | PreferredLanguageSource::Cookie
    )
}

pub(super) fn parse_article_research_metadata(
    preview_payload: Option<&str>,
) -> Option<ArticleResearchMetadata> {
    let payload: Value = serde_json::from_str(preview_payload?).ok()?;
    let research = payload.get("research")?.as_object()?;
    let source_count = research
        .get("source_count")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    if source_count == 0 {
        return None;
    }
    let mode_label = match research.get("mode").and_then(Value::as_str) {
        Some("manual") => "Requested research desk",
        _ => "Automatic research desk",
    };
    Some(ArticleResearchMetadata {
        mode_label: mode_label.to_string(),
        source_count,
    })
}

pub(super) async fn render_content_page(
    request: &WibbleRequest,
    slug: &str,
    source: Option<&str>,
    comments_page: Option<u64>,
    requested_language: Option<SupportedTranslationLanguage>,
) -> Result<Html<String>, Error> {
    let article = match query::load_content_page_article(request, slug).await? {
        query::ContentPageArticle::Ready(article) => *article,
        query::ContentPageArticle::Wait(wait_page) => return Ok(wait_page),
    };

    if policy::should_track_top_click(source, request.auth_user.is_some()) {
        query::increment_click_count(&request.state.db, &article.id).await?;
    }
    let interactions_open = article_accepts_public_interactions(&article);
    let comment_page = comments::load_comment_page(
        &request.state.db,
        &article.id,
        comments_page,
        interactions_open,
    )
    .await?;
    let user_vote = query::load_user_vote(
        &request.state.db,
        &article.id,
        request.auth_user.as_ref(),
        interactions_open,
    )
    .await?;

    let public_article = article.published && !article.flagged;
    let image_id = article.image_id.clone().unwrap_or_default();
    let markdown = article
        .markdown
        .as_deref()
        .ok_or(Error::NotFound(Some(format!(
            "Markdown for content {} not found",
            article.id
        ))))?;
    let source_article = ArticleSourceText {
        article_id: &article.id,
        title: &article.title,
        description: &article.description,
        markdown,
    };
    let mut available_translations =
        cached_translation_languages(&request.state.db, source_article).await?;
    let mut language_selection = resolve_article_language(
        requested_language,
        request.saved_article_language,
        request.browser_translation_language,
        &available_translations,
    );
    let translated_article =
        if language_selection.served_language.code != language_selection.source_language.code {
            load_cached_article_translation(
                &request.state.db,
                source_article,
                language_selection.served_language,
            )
            .await?
        } else {
            None
        };
    if translated_article.is_none()
        && language_selection.served_language.code != language_selection.source_language.code
    {
        available_translations
            .retain(|language| language.code != language_selection.served_language.code);
        language_selection = resolve_article_language(
            requested_language,
            request.saved_article_language,
            request.browser_translation_language,
            &available_translations,
        );
    }
    if language_selection.translation_requested && !language_selection.translation_available {
        request_article_translation(
            request.state.clone(),
            article.id.clone(),
            language_selection.preferred_language,
            request_source_from_preferred_language(language_selection.preferred_language_source),
            request.requester_tier,
            request.rate_limit_key.clone(),
        )
        .await;
    }
    let rendered_title = translated_article
        .as_ref()
        .map_or(article.title.as_str(), |translation| {
            translation.title.as_str()
        });
    let rendered_description = translated_article
        .as_ref()
        .map_or(article.description.as_str(), |translation| {
            translation.description.as_str()
        });
    let rendered_markdown = translated_article
        .as_ref()
        .map_or(markdown, |translation| translation.markdown.as_str());
    let rendered_body = render::markdown_to_html(&render::strip_leading_description(
        rendered_markdown,
        rendered_description,
    ));
    let language_options = build_article_language_options(
        &article.slug,
        language_selection,
        request.browser_translation_language,
    );
    let research_metadata = ArticleJobService::new(request.state.clone())
        .finalize_job_state_for_article(&article.id)
        .await?
        .and_then(|job| parse_article_research_metadata(job.preview_payload.as_deref()));
    let mut template = request.template("content").await;
    template
        .insert("id", &article.id)
        .insert("slug", &article.slug)
        .insert("created_at", &article.created_at.format("%F").to_string())
        .insert("description", rendered_description)
        .insert("image_id", &image_id)
        .insert("title", rendered_title)
        .insert("body", &rendered_body)
        .insert(
            "page_language_code",
            language_selection.served_language.code,
        )
        .insert(
            "page_language_name",
            language_selection.served_language.name,
        )
        .insert(
            "article_source_language_code",
            language_selection.source_language.code,
        )
        .insert(
            "article_source_language_name",
            language_selection.source_language.name,
        )
        .insert(
            "preferred_article_language_code",
            language_selection.preferred_language.code,
        )
        .insert(
            "preferred_article_language_name",
            language_selection.preferred_language.name,
        )
        .insert(
            "preferred_article_language_source",
            match language_selection.preferred_language_source {
                PreferredLanguageSource::Explicit => "explicit",
                PreferredLanguageSource::Cookie => "cookie",
                PreferredLanguageSource::Browser => "browser",
                PreferredLanguageSource::ArticleSource => "source",
            },
        )
        .insert(
            "served_article_language_source",
            match language_selection.served_language_source {
                ServedLanguageSource::Preferred => "preferred",
                ServedLanguageSource::ArticleSource => "source",
                ServedLanguageSource::EnglishFallback => "english_fallback",
            },
        )
        .insert(
            "article_translation_requested",
            &language_selection.translation_requested,
        )
        .insert(
            "article_translation_available",
            &language_selection.translation_available,
        )
        .insert("article_language_options", &language_options)
        .insert(
            "article_language_menu_open",
            &(uses_manual_article_language_preference(
                language_selection.preferred_language_source,
            ) || (language_selection.translation_requested
                && !language_selection.translation_available)),
        )
        .insert(
            "can_edit",
            &request
                .auth_user
                .as_ref()
                .is_some_and(|u| can_edit_article(u, &article)),
        )
        .insert(
            "can_publish",
            &request
                .auth_user
                .as_ref()
                .is_some_and(|u| can_toggle_publish(u, &article)),
        )
        .insert("is_published", &article.published)
        .insert("vote_score", &article.votes)
        .insert("voting_open", &interactions_open)
        .insert(
            "can_vote",
            &(interactions_open && request.auth_user.is_some()),
        )
        .insert("user_vote", &user_vote)
        .insert("comments", &comment_page.comments)
        .insert("comment_count", &comment_page.comment_count)
        .insert("comments_open", &interactions_open)
        .insert(
            "can_comment",
            &(interactions_open && request.auth_user.is_some()),
        )
        .insert("comment_pager", &comment_page.pager);
    let has_research_metadata = research_metadata.is_some();
    template.insert("article_research_metadata_present", &has_research_metadata);
    if let Some(research_metadata) = research_metadata {
        template.insert("article_research_metadata", &research_metadata);
    }
    if uses_manual_article_language_preference(language_selection.preferred_language_source) {
        template.insert(
            "article_language_override_code",
            language_selection.preferred_language.code,
        );
    }
    if !public_article {
        template.insert("robots", "noindex,nofollow");
    }
    template.render()
}
