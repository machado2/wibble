use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};
use crate::llm::translate::default_translation_fallback_language;

const DEFAULT_SOURCE_LANGUAGE_CODE: &str = "en";
pub const AUTOMATIC_LANGUAGE_QUERY_VALUE: &str = "auto";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreferredLanguageSource {
    Explicit,
    Route,
    Cookie,
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
    explicit_language: Option<SupportedTranslationLanguage>,
    route_language: Option<SupportedTranslationLanguage>,
    saved_language: Option<SupportedTranslationLanguage>,
    browser_language: Option<SupportedTranslationLanguage>,
    available_translations: &[SupportedTranslationLanguage],
) -> ArticleLanguageSelection {
    let source_language = article_source_language();

    let (preferred_language, preferred_language_source) = match explicit_language {
        Some(language) => (language, PreferredLanguageSource::Explicit),
        None => match route_language {
            Some(language) => (language, PreferredLanguageSource::Route),
            None => match saved_language {
                Some(language) => (language, PreferredLanguageSource::Cookie),
                None => match browser_language {
                    Some(language) => (language, PreferredLanguageSource::Browser),
                    None => (source_language, PreferredLanguageSource::ArticleSource),
                },
            },
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

pub fn resolve_supported_language_preference(value: &str) -> Option<SupportedTranslationLanguage> {
    find_supported_translation_language(value).or_else(|| {
        value
            .split(['-', '_'])
            .next()
            .and_then(find_supported_translation_language)
    })
}

pub fn resolve_requested_article_language(
    value: Option<&str>,
) -> Option<Option<SupportedTranslationLanguage>> {
    match value {
        None => Some(None),
        Some(value) if value.eq_ignore_ascii_case(AUTOMATIC_LANGUAGE_QUERY_VALUE) => Some(None),
        Some(value) => resolve_supported_language_preference(value).map(Some),
    }
}

pub fn requested_article_language_query_value(
    language: Option<SupportedTranslationLanguage>,
) -> &'static str {
    language.map_or(AUTOMATIC_LANGUAGE_QUERY_VALUE, |language| language.code)
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;

    use super::{
        article_source_language, requested_article_language_query_value, resolve_article_language,
        resolve_requested_article_language, PreferredLanguageSource, ServedLanguageSource,
    };

    #[test]
    fn article_source_language_defaults_to_english() {
        assert_eq!(article_source_language().code, "en");
    }

    #[test]
    fn explicit_language_beats_browser_language() {
        let browser_language = find_supported_translation_language("fr");
        let available_translations = [find_supported_translation_language("pt").unwrap()];

        let selection = resolve_article_language(
            find_supported_translation_language("pt"),
            None,
            None,
            browser_language,
            &available_translations,
        );

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

        let selection =
            resolve_article_language(None, None, None, browser_language, &available_translations);

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

        let selection = resolve_article_language(None, None, None, browser_language, &[]);

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
    fn missing_explicit_language_falls_back_to_browser_preference() {
        let browser_language = find_supported_translation_language("de");
        let available_translations = [find_supported_translation_language("de").unwrap()];

        let selection =
            resolve_article_language(None, None, None, browser_language, &available_translations);

        assert_eq!(selection.preferred_language.code, "de");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Browser
        );
        assert_eq!(selection.served_language.code, "de");
    }

    #[test]
    fn saved_cookie_language_is_used_when_query_is_absent() {
        let saved_language = find_supported_translation_language("pt");
        let browser_language = find_supported_translation_language("fr");

        let selection = resolve_article_language(None, None, saved_language, browser_language, &[]);

        assert_eq!(selection.preferred_language.code, "pt");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Cookie
        );
    }

    #[test]
    fn route_language_beats_saved_cookie_and_browser_language() {
        let saved_language = find_supported_translation_language("pt");
        let browser_language = find_supported_translation_language("fr");

        let selection = resolve_article_language(
            None,
            find_supported_translation_language("es"),
            saved_language,
            browser_language,
            &[],
        );

        assert_eq!(selection.preferred_language.code, "es");
        assert_eq!(
            selection.preferred_language_source,
            PreferredLanguageSource::Route
        );
    }

    #[test]
    fn resolve_requested_article_language_normalizes_supported_aliases() {
        let requested = resolve_requested_article_language(Some("pt-BR")).unwrap();

        assert_eq!(requested.unwrap().code, "pt");
    }

    #[test]
    fn resolve_requested_article_language_allows_automatic_mode() {
        let requested = resolve_requested_article_language(Some("auto")).unwrap();

        assert!(requested.is_none());
    }

    #[test]
    fn resolve_requested_article_language_rejects_unknown_values() {
        assert!(resolve_requested_article_language(Some("klingon")).is_none());
    }

    #[test]
    fn requested_article_language_query_value_uses_canonical_codes() {
        let requested = find_supported_translation_language("pt");

        assert_eq!(requested_article_language_query_value(requested), "pt");
        assert_eq!(requested_article_language_query_value(None), "auto");
    }
}
