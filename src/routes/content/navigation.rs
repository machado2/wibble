use std::env;

use crate::llm::prompt_registry::SupportedTranslationLanguage;
use crate::services::site_paths::localized_path;

const ARTICLE_LANGUAGE_COOKIE_NAME: &str = "__article_lang";

pub(super) fn content_location(
    site_language: SupportedTranslationLanguage,
    slug: &str,
    lang: Option<&str>,
    anchor: Option<&str>,
) -> String {
    content_location_with_query(site_language, slug, None, None, lang, anchor)
}

pub(super) fn content_location_with_query(
    site_language: SupportedTranslationLanguage,
    slug: &str,
    source: Option<&str>,
    comments_page: Option<u64>,
    lang: Option<&str>,
    anchor: Option<&str>,
) -> String {
    let mut path = localized_path(site_language, &format!("/content/{}", slug));
    let mut query = Vec::new();
    if let Some(source) = source {
        query.push(format!("source={}", source));
    }
    if let Some(comments_page) = comments_page {
        query.push(format!("comments_page={}", comments_page));
    }
    if let Some(lang) = lang {
        query.push(format!("lang={}", lang));
    }
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    if let Some(anchor) = anchor {
        path.push('#');
        path.push_str(anchor);
    }
    path
}

fn article_language_cookie_path(site_language: SupportedTranslationLanguage, slug: &str) -> String {
    localized_path(site_language, &format!("/content/{}", slug))
}

fn article_language_cookie(
    site_language: SupportedTranslationLanguage,
    slug: &str,
    value: &str,
    max_age: u64,
) -> String {
    let secure = if env::var("SITE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string())
        .starts_with("https://")
    {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={}; Path={}; SameSite=Lax{}; Max-Age={}",
        ARTICLE_LANGUAGE_COOKIE_NAME,
        value,
        article_language_cookie_path(site_language, slug),
        secure,
        max_age
    )
}

fn clear_article_language_cookie(
    site_language: SupportedTranslationLanguage,
    slug: &str,
) -> String {
    article_language_cookie(site_language, slug, "", 0)
}

pub(super) fn article_language_cookie_header(
    site_language: SupportedTranslationLanguage,
    slug: &str,
    requested_language: Option<SupportedTranslationLanguage>,
    automatic_language_code: &str,
    update_requested: bool,
) -> Option<String> {
    if !update_requested {
        return None;
    }

    match requested_language {
        None => Some(clear_article_language_cookie(site_language, slug)),
        Some(language) => {
            if language.code == automatic_language_code {
                Some(clear_article_language_cookie(site_language, slug))
            } else {
                Some(article_language_cookie(
                    site_language,
                    slug,
                    language.code,
                    30 * 24 * 60 * 60,
                ))
            }
        }
    }
}
