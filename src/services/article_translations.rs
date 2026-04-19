use std::collections::{HashMap, HashSet};

use sea_orm::DatabaseConnection;

use crate::error::Error;
use crate::llm::prompt_registry::{supported_translation_languages, SupportedTranslationLanguage};
use crate::repositories::translations::{
    find_translations_for_hashes, translation_source_hash, StoredTranslation,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArticleSourceText<'a> {
    pub title: &'a str,
    pub description: &'a str,
    pub markdown: &'a str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CachedArticleTranslation {
    pub language: SupportedTranslationLanguage,
    pub title: String,
    pub description: String,
    pub markdown: String,
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

fn assemble_cached_article_translation(
    source_hashes: &[String],
    translations: &[StoredTranslation],
    language: SupportedTranslationLanguage,
) -> Option<CachedArticleTranslation> {
    let translation_map = translations
        .iter()
        .filter(|translation| translation.language.code == language.code)
        .map(|translation| (translation.source_hash.as_str(), translation.text.as_str()))
        .collect::<HashMap<_, _>>();

    let title = translation_map.get(source_hashes[0].as_str())?;
    let description = translation_map.get(source_hashes[1].as_str())?;
    let markdown = translation_map.get(source_hashes[2].as_str())?;

    Some(CachedArticleTranslation {
        language,
        title: (*title).to_string(),
        description: (*description).to_string(),
        markdown: (*markdown).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;
    use crate::repositories::translations::StoredTranslation;

    use super::{
        assemble_cached_article_translation, complete_cached_languages, source_hashes,
        ArticleSourceText,
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
}
