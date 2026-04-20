use std::env;

use crate::llm::prompt_registry::SupportedTranslationLanguage;

const ARTICLE_LANGUAGE_COOKIE_NAME: &str = "__article_lang";

pub(super) fn content_location(slug: &str, lang: Option<&str>, anchor: Option<&str>) -> String {
    content_location_with_query(slug, None, None, lang, anchor)
}

pub(super) fn content_location_with_query(
    slug: &str,
    source: Option<&str>,
    comments_page: Option<u64>,
    lang: Option<&str>,
    anchor: Option<&str>,
) -> String {
    let mut path = format!("/content/{}", slug);
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

fn article_language_cookie_path(slug: &str) -> String {
    format!("/content/{}", slug)
}

fn article_language_cookie(slug: &str, value: &str, max_age: u64) -> String {
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
        article_language_cookie_path(slug),
        secure,
        max_age
    )
}

fn clear_article_language_cookie(slug: &str) -> String {
    article_language_cookie(slug, "", 0)
}

pub(super) fn article_language_cookie_header(
    slug: &str,
    requested_language: Option<SupportedTranslationLanguage>,
    automatic_language_code: &str,
    update_requested: bool,
) -> Option<String> {
    if !update_requested {
        return None;
    }

    match requested_language {
        None => Some(clear_article_language_cookie(slug)),
        Some(language) => {
            if language.code == automatic_language_code {
                Some(clear_article_language_cookie(slug))
            } else {
                Some(article_language_cookie(
                    slug,
                    language.code,
                    30 * 24 * 60 * 60,
                ))
            }
        }
    }
}
