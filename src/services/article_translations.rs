use std::collections::{HashMap, HashSet};

use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::app_state::AppState;
use crate::error::Error;
use crate::llm::prompt_registry::{supported_translation_languages, SupportedTranslationLanguage};
use crate::llm::translate::Translate;
use crate::repositories::translations::{
    find_translations_for_hashes, save_translation, translation_source_hash, StoredTranslation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArticleSourceText<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub markdown: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedArticleSourceText {
    pub title: String,
    pub description: String,
    pub markdown: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedArticleTranslation {
    pub language: SupportedTranslationLanguage,
    pub title: String,
    pub description: String,
    pub markdown: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CachedArticleTranslationFields {
    title: Option<String>,
    description: Option<String>,
    markdown: Option<String>,
}

impl OwnedArticleSourceText {
    pub fn as_ref(&self) -> ArticleSourceText<'_> {
        ArticleSourceText {
            title: &self.title,
            description: &self.description,
            markdown: &self.markdown,
        }
    }
}

pub fn article_translation_job_key(
    article_id: &str,
    language: SupportedTranslationLanguage,
) -> String {
    format!("{}:{}", article_id, language.code)
}

pub async fn spawn_missing_article_translation(
    state: AppState,
    article_id: String,
    source: OwnedArticleSourceText,
    language: SupportedTranslationLanguage,
) {
    let job_key = article_translation_job_key(&article_id, language);
    if !state
        .try_mark_translation_generation_started(&job_key)
        .await
    {
        return;
    }

    tokio::spawn(async move {
        if let Err(err) =
            ensure_cached_article_translation(&state.llm, &state.db, source.as_ref(), language)
                .await
        {
            warn!(
                article_id = %article_id,
                language = language.code,
                error = %err,
                "Failed to generate cached article translation"
            );
        }
        state.mark_translation_generation_finished(&job_key).await;
    });
}

pub async fn cached_translation_languages(
    db: &DatabaseConnection,
    source: ArticleSourceText<'_>,
) -> Result<Vec<SupportedTranslationLanguage>, Error> {
    let hashes = source_hashes(source);
    let translations = find_translations_for_hashes(db, &hashes).await?;

    Ok(complete_cached_languages(&hashes, &translations))
}

pub async fn load_cached_article_translation(
    db: &DatabaseConnection,
    source: ArticleSourceText<'_>,
    language: SupportedTranslationLanguage,
) -> Result<Option<CachedArticleTranslation>, Error> {
    let hashes = source_hashes(source);
    let translations = find_translations_for_hashes(db, &hashes).await?;

    Ok(assemble_cached_article_translation(
        &hashes,
        &translations,
        language,
    ))
}

pub async fn ensure_cached_article_translation<T: Translate>(
    translator: &T,
    db: &DatabaseConnection,
    source: ArticleSourceText<'_>,
    language: SupportedTranslationLanguage,
) -> Result<CachedArticleTranslation, Error> {
    let hashes = source_hashes(source);
    let translations = find_translations_for_hashes(db, &hashes).await?;
    let cached_fields = cached_translation_fields(&hashes, &translations, language);

    let title = match cached_fields.title {
        Some(title) => title,
        None => {
            let title = translator.translate(source.title, language).await?;
            save_translation(db, source.title, language, &title).await?;
            title
        }
    };
    let description = match cached_fields.description {
        Some(description) => description,
        None => {
            let description = translator.translate(source.description, language).await?;
            save_translation(db, source.description, language, &description).await?;
            description
        }
    };
    let markdown = match cached_fields.markdown {
        Some(markdown) => markdown,
        None => {
            let markdown = translator.translate(source.markdown, language).await?;
            save_translation(db, source.markdown, language, &markdown).await?;
            markdown
        }
    };

    Ok(CachedArticleTranslation {
        language,
        title,
        description,
        markdown,
    })
}

fn source_hashes(source: ArticleSourceText<'_>) -> Vec<String> {
    vec![
        translation_source_hash(source.title),
        translation_source_hash(source.description),
        translation_source_hash(source.markdown),
    ]
}

fn complete_cached_languages(
    source_hashes: &[String],
    translations: &[StoredTranslation],
) -> Vec<SupportedTranslationLanguage> {
    let mut by_language = HashMap::<&'static str, HashSet<&str>>::new();
    for translation in translations {
        by_language
            .entry(translation.language.code)
            .or_default()
            .insert(translation.source_hash.as_str());
    }

    supported_translation_languages()
        .iter()
        .copied()
        .filter(|language| {
            by_language.get(language.code).is_some_and(|hashes| {
                source_hashes
                    .iter()
                    .all(|hash| hashes.contains(hash.as_str()))
            })
        })
        .collect()
}

fn cached_translation_fields(
    source_hashes: &[String],
    translations: &[StoredTranslation],
    language: SupportedTranslationLanguage,
) -> CachedArticleTranslationFields {
    let translation_map = translations
        .iter()
        .filter(|translation| translation.language.code == language.code)
        .map(|translation| (translation.source_hash.as_str(), translation.text.as_str()))
        .collect::<HashMap<_, _>>();

    CachedArticleTranslationFields {
        title: translation_map
            .get(source_hashes[0].as_str())
            .map(|text| (*text).to_string()),
        description: translation_map
            .get(source_hashes[1].as_str())
            .map(|text| (*text).to_string()),
        markdown: translation_map
            .get(source_hashes[2].as_str())
            .map(|text| (*text).to_string()),
    }
}

fn assemble_cached_article_translation(
    source_hashes: &[String],
    translations: &[StoredTranslation],
    language: SupportedTranslationLanguage,
) -> Option<CachedArticleTranslation> {
    let cached_fields = cached_translation_fields(source_hashes, translations, language);
    let title = cached_fields.title?;
    let description = cached_fields.description?;
    let markdown = cached_fields.markdown?;

    Some(CachedArticleTranslation {
        language,
        title,
        description,
        markdown,
    })
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::repositories::translations::StoredTranslation;

    use super::{
        article_translation_job_key, assemble_cached_article_translation,
        cached_translation_fields, complete_cached_languages, source_hashes, ArticleSourceText,
    };

    fn source_text() -> ArticleSourceText<'static> {
        ArticleSourceText {
            title: "Report",
            description: "Brief summary",
            markdown: "Brief summary\n\nBody paragraph",
        }
    }

    #[test]
    fn complete_cached_languages_requires_all_article_fields() {
        let source = source_text();
        let hashes = source_hashes(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let incomplete = vec![
            StoredTranslation {
                source_hash: hashes[0].clone(),
                language: portuguese,
                text: "Relatorio".to_string(),
            },
            StoredTranslation {
                source_hash: hashes[1].clone(),
                language: portuguese,
                text: "Resumo".to_string(),
            },
        ];

        assert!(complete_cached_languages(&hashes, &incomplete).is_empty());
    }

    #[test]
    fn complete_cached_languages_returns_supported_languages_with_full_coverage() {
        let source = source_text();
        let hashes = source_hashes(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let complete = hashes
            .iter()
            .map(|hash| StoredTranslation {
                source_hash: hash.clone(),
                language: portuguese,
                text: format!("translated:{hash}"),
            })
            .collect::<Vec<_>>();

        let languages = complete_cached_languages(&hashes, &complete);

        assert_eq!(languages, vec![portuguese]);
    }

    #[test]
    fn assemble_cached_article_translation_uses_field_hashes() {
        let source = source_text();
        let hashes = source_hashes(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let translations = vec![
            StoredTranslation {
                source_hash: hashes[1].clone(),
                language: portuguese,
                text: "Resumo".to_string(),
            },
            StoredTranslation {
                source_hash: hashes[2].clone(),
                language: portuguese,
                text: "Resumo\n\nCorpo".to_string(),
            },
            StoredTranslation {
                source_hash: hashes[0].clone(),
                language: portuguese,
                text: "Relatorio".to_string(),
            },
        ];

        let cached =
            assemble_cached_article_translation(&hashes, &translations, portuguese).unwrap();

        assert_eq!(cached.title, "Relatorio");
        assert_eq!(cached.description, "Resumo");
        assert_eq!(cached.markdown, "Resumo\n\nCorpo");
    }

    #[test]
    fn cached_translation_fields_preserve_partial_rows() {
        let source = source_text();
        let hashes = source_hashes(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let translations = vec![StoredTranslation {
            source_hash: hashes[0].clone(),
            language: portuguese,
            text: "Relatorio".to_string(),
        }];

        let cached_fields = cached_translation_fields(&hashes, &translations, portuguese);

        assert_eq!(cached_fields.title.as_deref(), Some("Relatorio"));
        assert!(cached_fields.description.is_none());
        assert!(cached_fields.markdown.is_none());
    }

    #[test]
    fn article_translation_job_key_is_per_article_and_language() {
        let portuguese = find_supported_translation_language("pt").unwrap();

        assert_eq!(
            article_translation_job_key("article-1", portuguese),
            "article-1:pt"
        );
    }
}
