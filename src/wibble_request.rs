use std::collections::HashMap;
use std::env;
use std::error::Error as StdError;
use std::sync::Arc;
use std::sync::RwLock;

use axum::extract::{FromRef, FromRequestParts, Query};
use axum::response::Html;
use http::header::{ACCEPT_LANGUAGE, USER_AGENT};
use http::request::Parts;
use serde::Serialize;
use sha2::{Digest, Sha256};
use tera::{Context, Tera};
use tracing::log;

use crate::app_state::AppState;
use crate::auth::{extract_auth_token, AuthUser};
use crate::error::Error;
use crate::llm::prompt_registry::SupportedTranslationLanguage;
use crate::llm::translate::detect_browser_translation_language;
use crate::rate_limit::RequesterTier;
use crate::services::article_language::resolve_supported_language_preference;

const ARTICLE_LANGUAGE_COOKIE_NAME: &str = "__article_lang";

#[derive(Debug, Clone)]
pub struct WibbleRequest
where
    Self: Send + Sync,
{
    pub state: AppState,
    pub style: String,
    pub request_path: String,
    pub auth_user: Option<AuthUser>,
    pub requester_tier: RequesterTier,
    pub rate_limit_key: String,
    pub browser_translation_language: Option<SupportedTranslationLanguage>,
    pub saved_article_language: Option<SupportedTranslationLanguage>,
}

pub struct Template {
    tera: Arc<RwLock<Tera>>,
    name: String,
    context: Context,
    auto_reload: bool,
}

impl Template {
    pub fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) -> &mut Self {
        self.context.insert(key, val);
        self
    }
    pub fn render(&self) -> Result<Html<String>, Error> {
        if self.auto_reload {
            if let Ok(mut tera) = self.tera.write() {
                if let Err(e) = tera.full_reload() {
                    log::warn!("Template reload failed: {}", e);
                }
            }
        }
        let s = match self.tera.read() {
            Ok(tera) => tera.render(&self.name, &self.context),
            Err(e) => {
                log::error!("Template lock poisoned: {}", e);
                return Err(Error::Template(tera::Error::msg("Template lock poisoned")));
            }
        };
        match s {
            Ok(s) => Ok(Html(s)),
            Err(e) => {
                log::error!("Template error: {}", e);
                if let Some(source) = e.source() {
                    log::error!("Template error source: {}", source);
                }
                Err(Error::Template(e))
            }
        }
    }
}

impl WibbleRequest {
    fn get_site_url() -> String {
        env::var("SITE_URL")
            .ok()
            .map(|url| url.trim().trim_end_matches('/').to_string())
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| "https://wibble.news".to_string())
    }

    pub async fn template(&self, name: &str) -> Template {
        let mut context = tera::Context::new();
        let style = format!("/{}.css", self.style);
        let busted_style = self
            .state
            .bust_dir
            .get(&style)
            .map(|h| format!("{}?{}", style, h))
            .unwrap_or(style);
        let site_url = Self::get_site_url();
        let canonical_url = format!("{}{}", site_url, self.request_path);
        context.insert("style", &busted_style);
        context.insert("site_url", &site_url);
        context.insert("canonical_url", &canonical_url);
        context.insert("text_create_new_article", "Draft article");
        if let Some(language) = self.browser_translation_language {
            context.insert("browser_translation_language_code", language.code);
            context.insert("browser_translation_language_name", language.name);
        }
        if let Some(ref user) = self.auth_user {
            context.insert("auth_user", user);
            context.insert("is_admin", &user.is_admin());
        }
        Template {
            name: format!("{}.html", name),
            context,
            tera: Arc::clone(&self.state.tera),
            auto_reload: self.state.template_auto_reload,
        }
    }

    fn browser_translation_language_from_headers(
        headers: &http::HeaderMap,
    ) -> Option<SupportedTranslationLanguage> {
        detect_browser_translation_language(
            headers
                .get(ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok()),
        )
    }

    fn saved_article_language_from_headers(
        headers: &http::HeaderMap,
    ) -> Option<SupportedTranslationLanguage> {
        let cookie_header = headers.get(http::header::COOKIE)?.to_str().ok()?;
        cookie_header.split(';').map(str::trim).find_map(|cookie| {
            cookie
                .strip_prefix(&format!("{}=", ARTICLE_LANGUAGE_COOKIE_NAME))
                .and_then(resolve_supported_language_preference)
        })
    }

    fn client_ip_from_headers(headers: &http::HeaderMap) -> Option<String> {
        ["cf-connecting-ip", "x-real-ip", "x-forwarded-for"]
            .into_iter()
            .find_map(|header_name| {
                headers
                    .get(header_name)
                    .and_then(|value| value.to_str().ok())
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(|value| {
                        if header_name == "x-forwarded-for" {
                            value.split(',').next().unwrap_or(value).trim().to_string()
                        } else {
                            value.to_string()
                        }
                    })
            })
    }

    fn anonymous_rate_limit_key_from_headers(headers: &http::HeaderMap) -> String {
        let ip = Self::client_ip_from_headers(headers).unwrap_or_else(|| "unknown-ip".to_string());
        let user_agent = headers
            .get(USER_AGENT)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown-agent");
        let accept_language = headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown-language");
        let fingerprint = format!("{}|{}|{}", ip, user_agent, accept_language);
        format!("anon:{:x}", Sha256::digest(fingerprint.as_bytes()))
    }

    fn requester_tier_for(auth_user: Option<&AuthUser>) -> RequesterTier {
        match auth_user {
            Some(user) if user.is_admin() => RequesterTier::Admin,
            Some(_) => RequesterTier::Authenticated,
            None => RequesterTier::Anonymous,
        }
    }
}

impl<S> FromRequestParts<S> for WibbleRequest
where
    AppState: FromRef<S>,
    S: Sync + Send + 'static,
    WibbleRequest: Sync + Send + 'static,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let query = Query::<HashMap<String, String>>::try_from_uri(&parts.uri).ok();
        let style = query
            .and_then(|q| q.get("theme").cloned())
            .unwrap_or("style".to_string());
        let request_path = parts.uri.path().to_string();
        let state = AppState::from_ref(state);
        let browser_translation_language =
            WibbleRequest::browser_translation_language_from_headers(&parts.headers);
        let saved_article_language =
            WibbleRequest::saved_article_language_from_headers(&parts.headers);
        let auth_user = if let Some(token) = extract_auth_token(parts) {
            state.jwks_client.validate_token(&token).await.ok()
        } else {
            None
        };
        let requester_tier = WibbleRequest::requester_tier_for(auth_user.as_ref());
        let rate_limit_key = auth_user.as_ref().map_or_else(
            || WibbleRequest::anonymous_rate_limit_key_from_headers(&parts.headers),
            |user| format!("user:{}", user.email),
        );
        Ok(WibbleRequest {
            state,
            style,
            request_path,
            auth_user,
            requester_tier,
            rate_limit_key,
            browser_translation_language,
            saved_article_language,
        })
    }
}

#[cfg(test)]
mod tests {
    use http::{
        header::{ACCEPT_LANGUAGE, USER_AGENT},
        HeaderMap, HeaderValue,
    };

    use super::WibbleRequest;

    #[test]
    fn browser_translation_language_from_headers_reads_supported_language() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("es-MX,pt-BR;q=0.8,en;q=0.5"),
        );

        let language = WibbleRequest::browser_translation_language_from_headers(&headers).unwrap();

        assert_eq!(language.code, "es");
    }

    #[test]
    fn browser_translation_language_from_headers_ignores_unsupported_languages() {
        let mut headers = HeaderMap::new();
        headers.insert(
            ACCEPT_LANGUAGE,
            HeaderValue::from_static("zh-CN,ja;q=0.8,it;q=0.5"),
        );

        let language = WibbleRequest::browser_translation_language_from_headers(&headers).unwrap();

        assert_eq!(language.code, "it");
    }

    #[test]
    fn saved_article_language_from_headers_reads_supported_cookie_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::COOKIE,
            HeaderValue::from_static("__article_lang=pt-BR; other=value"),
        );

        let language = WibbleRequest::saved_article_language_from_headers(&headers).unwrap();

        assert_eq!(language.code, "pt");
    }

    #[test]
    fn saved_article_language_from_headers_ignores_unsupported_cookie_value() {
        let mut headers = HeaderMap::new();
        headers.insert(
            http::header::COOKIE,
            HeaderValue::from_static("__article_lang=klingon"),
        );

        let language = WibbleRequest::saved_article_language_from_headers(&headers);

        assert!(language.is_none());
    }

    #[test]
    fn anonymous_rate_limit_key_from_headers_is_stable_for_same_fingerprint() {
        let mut headers = HeaderMap::new();
        headers.insert("x-real-ip", HeaderValue::from_static("203.0.113.5"));
        headers.insert(USER_AGENT, HeaderValue::from_static("TestBrowser/1.0"));
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));

        let first = WibbleRequest::anonymous_rate_limit_key_from_headers(&headers);
        let second = WibbleRequest::anonymous_rate_limit_key_from_headers(&headers);

        assert_eq!(first, second);
        assert!(first.starts_with("anon:"));
    }

    #[test]
    fn anonymous_rate_limit_key_changes_when_ip_changes() {
        let mut headers_a = HeaderMap::new();
        headers_a.insert("x-real-ip", HeaderValue::from_static("203.0.113.5"));
        headers_a.insert(USER_AGENT, HeaderValue::from_static("TestBrowser/1.0"));

        let mut headers_b = HeaderMap::new();
        headers_b.insert("x-real-ip", HeaderValue::from_static("203.0.113.9"));
        headers_b.insert(USER_AGENT, HeaderValue::from_static("TestBrowser/1.0"));

        let first = WibbleRequest::anonymous_rate_limit_key_from_headers(&headers_a);
        let second = WibbleRequest::anonymous_rate_limit_key_from_headers(&headers_b);

        assert_ne!(first, second);
    }
}
