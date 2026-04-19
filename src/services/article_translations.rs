use std::collections::{HashMap, HashSet};

use sea_orm::DatabaseConnection;
use tracing::warn;

use crate::app_state::AppState;
use crate::error::Error;
use crate::llm::prompt_registry::{
    supported_translation_languages, translation_prompt, SupportedTranslationLanguage,
};
use crate::llm::translate::Translate;
use crate::repositories::translations::{
    find_translations_for_keys, save_translations, translation_cache_key, StoredTranslation,
    TranslationField, TranslationWrite,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArticleSourceText<'a> {
    pub article_id: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    pub markdown: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnedArticleSourceText {
    pub article_id: String,
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct MissingArticleTranslationFields {
    title: bool,
    description: bool,
    markdown: bool,
}

impl OwnedArticleSourceText {
    pub fn as_ref(&self) -> ArticleSourceText<'_> {
        ArticleSourceText {
            article_id: &self.article_id,
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
    source: OwnedArticleSourceText,
    language: SupportedTranslationLanguage,
) {
    let job_key = article_translation_job_key(&source.article_id, language);
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
                article_id = %source.article_id,
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
    let cache_keys = cache_keys(source);
    let translations = find_translations_for_keys(db, &cache_keys).await?;

    Ok(complete_cached_languages(&cache_keys, &translations))
}

pub async fn load_cached_article_translation(
    db: &DatabaseConnection,
    source: ArticleSourceText<'_>,
    language: SupportedTranslationLanguage,
) -> Result<Option<CachedArticleTranslation>, Error> {
    let cache_keys = cache_keys(source);
    let translations = find_translations_for_keys(db, &cache_keys).await?;

    Ok(assemble_cached_article_translation(
        &cache_keys,
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
    let cache_keys = cache_keys(source);
    let translations = find_translations_for_keys(db, &cache_keys).await?;
    let cached_fields = cached_translation_fields(&cache_keys, &translations, language);
    let missing_fields = MissingArticleTranslationFields {
        title: cached_fields.title.is_none(),
        description: cached_fields.description.is_none(),
        markdown: cached_fields.markdown.is_none(),
    };

    let title = match cached_fields.title {
        Some(title) => title,
        None => translator.translate(source.title, language).await?,
    };
    let description = match cached_fields.description {
        Some(description) => description,
        None => translator.translate(source.description, language).await?,
    };
    let markdown = match cached_fields.markdown {
        Some(markdown) => markdown,
        None => translator.translate(source.markdown, language).await?,
    };
    let writes = missing_translation_writes(
        &cache_keys,
        language,
        &missing_fields,
        [&title, &description, &markdown],
    );
    save_translations(db, &writes).await?;

    Ok(CachedArticleTranslation {
        language,
        title,
        description,
        markdown,
    })
}

fn translation_prompt_version() -> i32 {
    translation_prompt().version
}

fn cache_keys(source: ArticleSourceText<'_>) -> Vec<String> {
    vec![
        translation_cache_key(
            source.article_id,
            TranslationField::Title,
            source.title,
            translation_prompt_version(),
        ),
        translation_cache_key(
            source.article_id,
            TranslationField::Description,
            source.description,
            translation_prompt_version(),
        ),
        translation_cache_key(
            source.article_id,
            TranslationField::Markdown,
            source.markdown,
            translation_prompt_version(),
        ),
    ]
}

fn complete_cached_languages(
    cache_keys: &[String],
    translations: &[StoredTranslation],
) -> Vec<SupportedTranslationLanguage> {
    let mut by_language = HashMap::<&'static str, HashSet<&str>>::new();
    for translation in translations {
        by_language
            .entry(translation.language.code)
            .or_default()
            .insert(translation.cache_key.as_str());
    }

    supported_translation_languages()
        .iter()
        .copied()
        .filter(|language| {
            by_language
                .get(language.code)
                .is_some_and(|hashes| cache_keys.iter().all(|hash| hashes.contains(hash.as_str())))
        })
        .collect()
}

fn cached_translation_fields(
    cache_keys: &[String],
    translations: &[StoredTranslation],
    language: SupportedTranslationLanguage,
) -> CachedArticleTranslationFields {
    let translation_map = translations
        .iter()
        .filter(|translation| translation.language.code == language.code)
        .map(|translation| (translation.cache_key.as_str(), translation.text.as_str()))
        .collect::<HashMap<_, _>>();

    CachedArticleTranslationFields {
        title: translation_map
            .get(cache_keys[0].as_str())
            .map(|text| (*text).to_string()),
        description: translation_map
            .get(cache_keys[1].as_str())
            .map(|text| (*text).to_string()),
        markdown: translation_map
            .get(cache_keys[2].as_str())
            .map(|text| (*text).to_string()),
    }
}

fn missing_translation_writes(
    cache_keys: &[String],
    language: SupportedTranslationLanguage,
    missing_fields: &MissingArticleTranslationFields,
    translated_texts: [&str; 3],
) -> Vec<TranslationWrite> {
    let missing = [
        missing_fields.title,
        missing_fields.description,
        missing_fields.markdown,
    ];
    cache_keys
        .iter()
        .zip(translated_texts)
        .zip(missing)
        .filter(|((_, _), missing)| *missing)
        .map(|((cache_key, text), _)| TranslationWrite {
            cache_key: cache_key.clone(),
            language,
            text: text.to_string(),
        })
        .collect()
}

fn assemble_cached_article_translation(
    cache_keys: &[String],
    translations: &[StoredTranslation],
    language: SupportedTranslationLanguage,
) -> Option<CachedArticleTranslation> {
    let cached_fields = cached_translation_fields(cache_keys, translations, language);
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
        article_translation_job_key, assemble_cached_article_translation, cache_keys,
        cached_translation_fields, complete_cached_languages, missing_translation_writes,
        ArticleSourceText, MissingArticleTranslationFields,
    };

    fn source_text() -> ArticleSourceText<'static> {
        ArticleSourceText {
            article_id: "article-1",
            title: "Report",
            description: "Brief summary",
            markdown: "Brief summary\n\nBody paragraph",
        }
    }

    #[test]
    fn complete_cached_languages_requires_all_article_fields() {
        let source = source_text();
        let cache_keys = cache_keys(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let incomplete = vec![
            StoredTranslation {
                cache_key: cache_keys[0].clone(),
                language: portuguese,
                text: "Relatorio".to_string(),
            },
            StoredTranslation {
                cache_key: cache_keys[1].clone(),
                language: portuguese,
                text: "Resumo".to_string(),
            },
        ];

        assert!(complete_cached_languages(&cache_keys, &incomplete).is_empty());
    }

    #[test]
    fn complete_cached_languages_returns_supported_languages_with_full_coverage() {
        let source = source_text();
        let cache_keys = cache_keys(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let complete = cache_keys
            .iter()
            .map(|key| StoredTranslation {
                cache_key: key.clone(),
                language: portuguese,
                text: format!("translated:{key}"),
            })
            .collect::<Vec<_>>();

        let languages = complete_cached_languages(&cache_keys, &complete);

        assert_eq!(languages, vec![portuguese]);
    }

    #[test]
    fn assemble_cached_article_translation_uses_field_hashes() {
        let source = source_text();
        let cache_keys = cache_keys(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let translations = vec![
            StoredTranslation {
                cache_key: cache_keys[1].clone(),
                language: portuguese,
                text: "Resumo".to_string(),
            },
            StoredTranslation {
                cache_key: cache_keys[2].clone(),
                language: portuguese,
                text: "Resumo\n\nCorpo".to_string(),
            },
            StoredTranslation {
                cache_key: cache_keys[0].clone(),
                language: portuguese,
                text: "Relatorio".to_string(),
            },
        ];

        let cached =
            assemble_cached_article_translation(&cache_keys, &translations, portuguese).unwrap();

        assert_eq!(cached.title, "Relatorio");
        assert_eq!(cached.description, "Resumo");
        assert_eq!(cached.markdown, "Resumo\n\nCorpo");
    }

    #[test]
    fn cached_translation_fields_preserve_partial_rows() {
        let source = source_text();
        let cache_keys = cache_keys(source);
        let portuguese = find_supported_translation_language("pt").unwrap();

        let translations = vec![StoredTranslation {
            cache_key: cache_keys[0].clone(),
            language: portuguese,
            text: "Relatorio".to_string(),
        }];

        let cached_fields = cached_translation_fields(&cache_keys, &translations, portuguese);

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

    #[test]
    fn cache_keys_remain_distinct_when_source_text_matches() {
        let source = ArticleSourceText {
            article_id: "article-1",
            title: "Same text",
            description: "Same text",
            markdown: "Same text",
        };

        let cache_keys = cache_keys(source);

        assert_ne!(cache_keys[0], cache_keys[1]);
        assert_ne!(cache_keys[1], cache_keys[2]);
        assert_ne!(cache_keys[0], cache_keys[2]);
    }

    #[test]
    fn missing_translation_writes_only_stage_uncached_fields() {
        let source = source_text();
        let cache_keys = cache_keys(source);
        let portuguese = find_supported_translation_language("pt").unwrap();
        let writes = missing_translation_writes(
            &cache_keys,
            portuguese,
            &MissingArticleTranslationFields {
                title: true,
                description: false,
                markdown: true,
            },
            ["Titulo", "Resumo", "Corpo"],
        );

        assert_eq!(writes.len(), 2);
        assert_eq!(writes[0].cache_key, cache_keys[0]);
        assert_eq!(writes[0].text, "Titulo");
        assert_eq!(writes[1].cache_key, cache_keys[2]);
        assert_eq!(writes[1].text, "Corpo");
    }
}
