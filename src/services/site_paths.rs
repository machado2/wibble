use std::env;

use http::HeaderMap;

use crate::llm::prompt_registry::{
    find_supported_translation_language, SupportedTranslationLanguage,
};

pub const SITE_LANGUAGE_COOKIE_NAME: &str = "__site_lang";

fn english_site_language() -> SupportedTranslationLanguage {
    find_supported_translation_language("en").expect("English must remain a supported site locale")
}

fn portuguese_site_language() -> SupportedTranslationLanguage {
    find_supported_translation_language("pt")
        .expect("Portuguese must remain a supported site locale")
}

pub fn supported_site_languages() -> [SupportedTranslationLanguage; 2] {
    [english_site_language(), portuguese_site_language()]
}

pub fn find_supported_site_language(value: &str) -> Option<SupportedTranslationLanguage> {
    supported_site_languages()
        .into_iter()
        .find(|language| language.code.eq_ignore_ascii_case(value))
}

pub fn detect_site_language_from_path(path: &str) -> Option<SupportedTranslationLanguage> {
    let first_segment = path
        .trim_start_matches('/')
        .split(['/', '?'])
        .next()
        .filter(|segment| !segment.is_empty())?;
    find_supported_site_language(first_segment)
}

pub fn site_relative_path(path_and_query: &str) -> String {
    for language in supported_site_languages() {
        let prefix = format!("/{}", language.code);
        if path_and_query == prefix || path_and_query == format!("{}/", prefix) {
            return "/".to_string();
        }
        if let Some(rest) = path_and_query.strip_prefix(&prefix) {
            if rest.is_empty() {
                return "/".to_string();
            }
            if rest.starts_with('/') {
                return rest.to_string();
            }
            if rest.starts_with('?') {
                return format!("/{}", rest);
            }
        }
    }

    if path_and_query.is_empty() {
        "/".to_string()
    } else if path_and_query.starts_with('/') {
        path_and_query.to_string()
    } else {
        format!("/{}", path_and_query)
    }
}

pub fn localized_root_path(language: SupportedTranslationLanguage) -> String {
    format!("/{}/", language.code)
}

pub fn locale_prefix(language: SupportedTranslationLanguage) -> String {
    format!("/{}", language.code)
}

pub fn localized_path(language: SupportedTranslationLanguage, path_and_query: &str) -> String {
    let relative = site_relative_path(path_and_query);
    if relative == "/" {
        return localized_root_path(language);
    }
    format!("/{}{}", language.code, relative)
}

pub fn site_language_cookie_header(language: SupportedTranslationLanguage) -> String {
    let secure = if env::var("SITE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string())
        .starts_with("https://")
    {
        "; Secure"
    } else {
        ""
    };

    format!(
        "{}={}; Path=/; SameSite=Lax{}; Max-Age={}",
        SITE_LANGUAGE_COOKIE_NAME,
        language.code,
        secure,
        30 * 24 * 60 * 60
    )
}

pub fn saved_site_language_from_headers(
    headers: &HeaderMap,
) -> Option<SupportedTranslationLanguage> {
    let cookie_header = headers.get(http::header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').map(str::trim).find_map(|cookie| {
        cookie
            .strip_prefix(&format!("{}=", SITE_LANGUAGE_COOKIE_NAME))
            .and_then(find_supported_site_language)
    })
}

#[cfg(test)]
mod tests {
    use super::{
        detect_site_language_from_path, find_supported_site_language, localized_path,
        localized_root_path, site_relative_path,
    };

    #[test]
    fn finds_supported_site_language_codes() {
        assert_eq!(find_supported_site_language("en").unwrap().code, "en");
        assert_eq!(find_supported_site_language("pt").unwrap().code, "pt");
        assert!(find_supported_site_language("es").is_none());
    }

    #[test]
    fn detects_locale_prefix_from_path() {
        assert_eq!(
            detect_site_language_from_path("/pt/content/story")
                .unwrap()
                .code,
            "pt"
        );
        assert_eq!(
            detect_site_language_from_path("/en?search=test")
                .unwrap()
                .code,
            "en"
        );
        assert!(detect_site_language_from_path("/content/story").is_none());
    }

    #[test]
    fn strips_locale_prefix_before_relocalizing() {
        let english = find_supported_site_language("en").unwrap();
        assert_eq!(
            site_relative_path("/pt/content/story?lang=fr"),
            "/content/story?lang=fr"
        );
        assert_eq!(
            localized_path(english, "/pt/content/story?lang=fr"),
            "/en/content/story?lang=fr"
        );
        assert_eq!(localized_root_path(english), "/en/");
    }
}
