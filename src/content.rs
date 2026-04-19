use crate::error::Error;
use crate::llm::prompt_registry::{supported_translation_languages, SupportedTranslationLanguage};
use crate::permissions::{can_edit_article, can_toggle_publish};
use crate::services::article_language::{
    resolve_article_language, ArticleLanguageSelection, PreferredLanguageSource,
    ServedLanguageSource,
};
use crate::wibble_request::WibbleRequest;
use axum::response::Html;
use serde::Serialize;

mod comments;
mod policy;
mod query;
mod render;

pub use comments::{normalize_comment_body, normalize_comments_page};
pub use policy::{article_accepts_public_interactions, can_view_article};
pub use query::{find_article_by_slug, require_article_by_slug};

#[derive(Serialize)]
struct ArticleLanguageOption {
    href: String,
    label: String,
    note: String,
    active: bool,
}

fn article_language_href(slug: &str, lang: Option<&str>) -> String {
    let mut path = format!("/content/{}", slug);
    if let Some(lang) = lang {
        path.push_str("?lang=");
        path.push_str(lang);
    }
    path
}

fn build_article_language_options(
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

fn uses_manual_article_language_preference(source: PreferredLanguageSource) -> bool {
    matches!(
        source,
        PreferredLanguageSource::Explicit | PreferredLanguageSource::Cookie
    )
}

#[allow(async_fn_in_trait)]
pub trait GetContent {
    async fn get_content(
        &self,
        slug: &str,
        source: Option<&str>,
        comments_page: Option<u64>,
        requested_language: Option<&str>,
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
        requested_language: Option<&str>,
    ) -> Result<Html<String>, Error> {
        let article = match query::load_content_page_article(self, slug).await? {
            query::ContentPageArticle::Ready(article) => *article,
            query::ContentPageArticle::Wait(wait_page) => return Ok(wait_page),
        };

        if policy::should_track_top_click(source, self.auth_user.is_some()) {
            query::increment_click_count(&self.state.db, &article.id).await?;
        }
        let interactions_open = article_accepts_public_interactions(&article);
        let comment_page = comments::load_comment_page(
            &self.state.db,
            &article.id,
            comments_page,
            interactions_open,
        )
        .await?;
        let user_vote = query::load_user_vote(
            &self.state.db,
            &article.id,
            self.auth_user.as_ref(),
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
        let rendered_body = render::markdown_to_html(&render::strip_leading_description(
            markdown,
            &article.description,
        ));
        let language_selection = resolve_article_language(
            requested_language,
            self.saved_article_language,
            self.browser_translation_language,
            &[],
        );
        let language_options = build_article_language_options(
            &article.slug,
            language_selection,
            self.browser_translation_language,
        );
        let mut template = self.template("content").await;
        template
            .insert("id", &article.id)
            .insert("slug", &article.slug)
            .insert("created_at", &article.created_at.format("%F").to_string())
            .insert("description", &article.description)
            .insert("image_id", &image_id)
            .insert("title", &article.title)
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
                &self
                    .auth_user
                    .as_ref()
                    .is_some_and(|u| can_edit_article(u, &article)),
            )
            .insert(
                "can_publish",
                &self
                    .auth_user
                    .as_ref()
                    .is_some_and(|u| can_toggle_publish(u, &article)),
            )
            .insert("is_published", &article.published)
            .insert("vote_score", &article.votes)
            .insert("voting_open", &interactions_open)
            .insert("can_vote", &(interactions_open && self.auth_user.is_some()))
            .insert("user_vote", &user_vote)
            .insert("comments", &comment_page.comments)
            .insert("comment_count", &comment_page.comment_count)
            .insert("comments_open", &interactions_open)
            .insert(
                "can_comment",
                &(interactions_open && self.auth_user.is_some()),
            )
            .insert("comment_pager", &comment_page.pager);
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
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::services::article_language::resolve_article_language;

    use super::{article_language_href, build_article_language_options};

    #[test]
    fn article_language_href_uses_query_param_when_override_exists() {
        assert_eq!(
            article_language_href("story-slug", Some("pt")),
            "/content/story-slug?lang=pt"
        );
    }

    #[test]
    fn automatic_language_option_is_active_without_explicit_override() {
        let selection =
            resolve_article_language(None, None, find_supported_translation_language("pt"), &[]);

        let options = build_article_language_options(
            "story-slug",
            selection,
            find_supported_translation_language("pt"),
        );

        assert!(options[0].active);
        assert_eq!(options[0].label, "Automatic");
    }

    #[test]
    fn requested_language_option_stays_active_while_falling_back() {
        let selection = resolve_article_language(Some("pt-BR"), None, None, &[]);

        let options = build_article_language_options("story-slug", selection, None);
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
            find_supported_translation_language("pt"),
            find_supported_translation_language("fr"),
            &[],
        );

        let options = build_article_language_options(
            "story-slug",
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
            find_supported_translation_language("pt"),
            find_supported_translation_language("fr"),
            &[find_supported_translation_language("pt").unwrap()],
        );

        let options = build_article_language_options(
            "story-slug",
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
}
