use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};
use crate::llm::translate::default_translation_fallback_language;

const DEFAULT_SOURCE_LANGUAGE_CODE: &str = "en";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferredLanguageSource {
    Explicit,
    Browser,
    ArticleSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServedLanguageSource {
    Preferred,
    ArticleSource,
    EnglishFallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArticleLanguageSelection {
    pub source_language: SupportedTranslationLanguage,
    pub preferred_language: SupportedTranslationLanguage,
    pub preferred_language_source: PreferredLanguageSource,
    pub served_language: SupportedTranslationLanguage,
    pub served_language_source: ServedLanguageSource,
    pub translation_requested: bool,
    pub translation_available: bool,
}

pub fn article_source_language() -> SupportedTranslationLanguage {
    find_supported_translation_language(DEFAULT_SOURCE_LANGUAGE_CODE)
        .expect("English must remain the default source article language")
}

pub fn resolve_article_language(
    explicit_language: Option<&str>,
    browser_language: Option<SupportedTranslationLanguage>,
    available_translations: &[SupportedTranslationLanguage],
) -> ArticleLanguageSelection {
    let source_language = article_source_language();
    let explicit_language = explicit_language.and_then(resolve_supported_language_preference);

    let (preferred_language, preferred_language_source) = match explicit_language {
        Some(language) => (language, PreferredLanguageSource::Explicit),
        None => match browser_language {
            Some(language) => (language, PreferredLanguageSource::Browser),
            None => (source_language, PreferredLanguageSource::ArticleSource),
        },
    };

    let translation_requested = preferred_language.code != source_language.code;
    let translation_available = !translation_requested
        || available_translations
            .iter()
            .any(|language| language.code == preferred_language.code);

    let fallback_language = default_translation_fallback_language(source_language);
    let english_fallback_available = fallback_language.code == source_language.code
        || available_translations
            .iter()
            .any(|language| language.code == fallback_language.code);

    let (served_language, served_language_source) = if translation_available {
        (preferred_language, ServedLanguageSource::Preferred)
    } else if fallback_language.code != source_language.code && english_fallback_available {
        (fallback_language, ServedLanguageSource::EnglishFallback)
    } else if fallback_language.code == source_language.code {
        (source_language, ServedLanguageSource::EnglishFallback)
    } else {
        (source_language, ServedLanguageSource::ArticleSource)
    };

    ArticleLanguageSelection {
        source_language,
        preferred_language,
        preferred_language_source,
        served_language,
        served_language_source,
        translation_requested,
        translation_available,
    }
}

fn resolve_supported_language_preference(value: &str) -> Option<SupportedTranslationLanguage> {
    find_supported_translation_language(value).or_else(|| {
        value
            .split(['-', '_'])
            .next()
            .and_then(find_supported_translation_language)
    })
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;

    use super::{
        article_source_language, resolve_article_language, PreferredLanguageSource,
        ServedLanguageSource,
    };

    #[test]
    fn article_source_language_defaults_to_english() {
        assert_eq!(article_source_language().code, "en");
    }

    #[test]
    fn explicit_language_beats_browser_language() {
        let browser_language = find_supported_translation_language("fr");
        let available_translations = [find_supported_translation_language("pt").unwrap()];

        let selection =
            resolve_article_language(Some("pt-BR"), browser_language, &available_translations);

        assert_eq!(selection.preferred_language.code, "pt");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Explicit
        );
        assert_eq!(selection.served_language.code, "pt");
        assert_eq!(
            selection.served_language_source,
            ServedLanguageSource::Preferred
        );
    }

    #[test]
    fn browser_language_is_used_when_translation_is_available() {
        let browser_language = find_supported_translation_language("es");
        let available_translations = [find_supported_translation_language("es").unwrap()];

        let selection = resolve_article_language(None, browser_language, &available_translations);

        assert_eq!(selection.preferred_language.code, "es");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Browser
        );
        assert_eq!(selection.served_language.code, "es");
    }

    #[test]
    fn missing_translation_falls_back_to_source_article_immediately() {
        let browser_language = find_supported_translation_language("pt");

        let selection = resolve_article_language(None, browser_language, &[]);

        assert_eq!(selection.preferred_language.code, "pt");
        assert!(selection.translation_requested);
        assert!(!selection.translation_available);
        assert_eq!(selection.served_language.code, "en");
        assert_eq!(
            selection.served_language_source,
            ServedLanguageSource::EnglishFallback
        );
    }

    #[test]
    fn unsupported_explicit_language_falls_back_to_browser_preference() {
        let browser_language = find_supported_translation_language("de");
        let available_translations = [find_supported_translation_language("de").unwrap()];

        let selection =
            resolve_article_language(Some("klingon"), browser_language, &available_translations);

        assert_eq!(selection.preferred_language.code, "de");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Browser
        );
        assert_eq!(selection.served_language.code, "de");
    }
}
