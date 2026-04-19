use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
};
use sha2::{Digest, Sha256};

use crate::entities::{language, prelude::*, translation};
use crate::error::Error;
use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationField {
    Title,
    Description,
    Markdown,
}

impl TranslationField {
    fn as_str(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Description => "description",
            Self::Markdown => "markdown",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredTranslation {
    pub cache_key: String,
    pub language: SupportedTranslationLanguage,
    pub text: String,
}

pub fn translation_source_hash(source_text: &str) -> String {
    format!("{:x}", Sha256::digest(source_text.as_bytes()))
}

pub fn translation_cache_key(
    article_id: &str,
    field: TranslationField,
    source_text: &str,
    prompt_version: i32,
) -> String {
    stable_identifier(
        "translation-cache",
        &format!(
            "{}:{}:{}:{}",
            article_id,
            field.as_str(),
            translation_source_hash(source_text),
            prompt_version
        ),
    )
}

fn stable_identifier(namespace: &str, value: &str) -> String {
    let digest = format!(
        "{:x}",
        Sha256::digest(format!("{}:{}", namespace, value).as_bytes())
    );
    format!(
        "{}-{}-{}-{}-{}",
        &digest[0..8],
        &digest[8..12],
        &digest[12..16],
        &digest[16..20],
        &digest[20..32]
    )
}

fn language_row_id(language: SupportedTranslationLanguage) -> String {
    stable_identifier("translation-language", language.code)
}

fn translation_row_id(source_hash: &str, language: SupportedTranslationLanguage) -> String {
    stable_identifier(
        "translation-entry",
        &format!("{}:{}", source_hash, language.code),
    )
}

pub async fn find_translations_for_keys(
    db: &DatabaseConnection,
    cache_keys: &[String],
) -> Result<Vec<StoredTranslation>, Error> {
    if cache_keys.is_empty() {
        return Ok(Vec::new());
    }

    let rows = Translation::find()
        .filter(translation::Column::EnglishHash.is_in(cache_keys.iter().cloned()))
        .find_also_related(Language)
        .all(db)
        .await
        .map_err(|e| Error::Database(format!("Error loading cached translations: {}", e)))?;

    Ok(rows
        .into_iter()
        .filter_map(|(translation, language)| {
            let language = language?;
            let supported = find_supported_translation_language(&language.name)?;
            Some(StoredTranslation {
                cache_key: translation.english_hash,
                language: supported,
                text: translation.translation,
            })
        })
        .collect())
}

pub async fn save_translation(
    db: &DatabaseConnection,
    cache_key: &str,
    language: SupportedTranslationLanguage,
    translated_text: &str,
) -> Result<(), Error> {
    let language_id = language_row_id(language);
    let translation_id = translation_row_id(cache_key, language);

    if Language::find_by_id(language_id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking language cache row: {}", e)))?
        .is_none()
    {
        let language_row = language::ActiveModel {
            id: ActiveValue::set(language_id.clone()),
            name: ActiveValue::set(language.name.to_string()),
        };
        Language::insert(language_row)
            .exec(db)
            .await
            .map_err(|e| Error::Database(format!("Error inserting language cache row: {}", e)))?;
    }

    if let Some(existing) = Translation::find_by_id(translation_id.clone())
        .one(db)
        .await
        .map_err(|e| Error::Database(format!("Error checking cached translation row: {}", e)))?
    {
        let mut active: translation::ActiveModel = existing.into();
        active.english_hash = ActiveValue::set(cache_key.to_string());
        active.lang_id = ActiveValue::set(language_id);
        active.translation = ActiveValue::set(translated_text.to_string());
        active
            .update(db)
            .await
            .map_err(|e| Error::Database(format!("Error updating cached translation: {}", e)))?;
    } else {
        Translation::insert(translation::ActiveModel {
            id: ActiveValue::set(translation_id),
            english_hash: ActiveValue::set(cache_key.to_string()),
            lang_id: ActiveValue::set(language_id),
            translation: ActiveValue::set(translated_text.to_string()),
        })
        .exec(db)
        .await
        .map_err(|e| Error::Database(format!("Error inserting cached translation: {}", e)))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        stable_identifier, translation_cache_key, translation_source_hash, TranslationField,
    };

    #[test]
    fn translation_source_hash_is_stable() {
        let hash = translation_source_hash("Breaking: test bulletin");

        assert_eq!(hash.len(), 64);
        assert_eq!(hash, translation_source_hash("Breaking: test bulletin"));
    }

    #[test]
    fn stable_identifier_uses_uuid_like_format() {
        let id = stable_identifier("translation-language", "pt");

        assert_eq!(id.len(), 36);
        assert_eq!(&id[8..9], "-");
        assert_eq!(&id[13..14], "-");
        assert_eq!(&id[18..19], "-");
        assert_eq!(&id[23..24], "-");
    }

    #[test]
    fn translation_cache_key_is_field_specific() {
        let title_key = translation_cache_key("article-1", TranslationField::Title, "Same", 1);
        let description_key =
            translation_cache_key("article-1", TranslationField::Description, "Same", 1);

        assert_ne!(title_key, description_key);
    }

    #[test]
    fn translation_cache_key_changes_with_article_and_prompt_version() {
        let base = translation_cache_key("article-1", TranslationField::Title, "Same", 1);
        let other_article = translation_cache_key("article-2", TranslationField::Title, "Same", 1);
        let other_prompt = translation_cache_key("article-1", TranslationField::Title, "Same", 2);

        assert_ne!(base, other_article);
        assert_ne!(base, other_prompt);
    }
}
