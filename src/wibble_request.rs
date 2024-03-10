use std::collections::HashMap;
use std::error::Error as StdError;

use axum::async_trait;
use axum::extract::{FromRef, FromRequestParts, Query};
use axum::response::Html;
use http::request::Parts;
use serde::Serialize;
use tera::{Context, Tera};
use tracing::log;

use crate::app_state::AppState;
use crate::error::Error;
use crate::translate::Translator;

#[derive(Debug, Clone)]
pub struct WibbleRequest
where
    Self: Send + Sync,
{
    pub state: AppState,
    pub style: String,
    pub lang: String,
}

pub struct Template<'a> {
    tera: &'a Tera,
    name: String,
    context: Context,
}

impl<'a> Template<'a> {
    pub fn insert<T: Serialize + ?Sized, S: Into<String>>(&mut self, key: S, val: &T) -> &mut Self {
        self.context.insert(key, val);
        self
    }
    pub fn render(&self) -> Result<Html<String>, Error> {
        let s = self.tera.render(&self.name, &self.context);
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
    pub async fn template(&self, name: &str) -> Template {
        let mut context = tera::Context::new();
        let style = format!("/{}.css", self.style);
        let busted_style = self
            .state
            .bust_dir
            .get(&style)
            .map(|h| format!("{}?{}", style, h))
            .unwrap_or(style);
        context.insert("style", &busted_style);
        context.insert(
            "text_create_new_article",
            &self.translate("Create new article").await,
        );
        Template {
            name: format!("{}.html", name),
            context,
            tera: &self.state.tera,
        }
    }
}

// #[async_trait]
// impl<S> FromRequest<S> for WibbleRequest
// where
//     AppState: FromRef<S>,
//     S: Sync + Send,
// {
//     type Rejection = Error;
//
//     async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
//         let query = Query::<HashMap<String, String>>::try_from_uri(req.uri()).ok();
//         let lang = query
//             .and_then(|q| q.get("lang").cloned())
//             .or_else(|| {
//                 req.headers()
//                     .get("Accept-Language")
//                     .and_then(|h| h.to_str().ok())
//                     .map(String::from)
//             })
//             .unwrap_or("en-US".to_string());
//         Ok(WibbleRequest {
//             state: AppState::from_ref(state),
//             style: None,
//             lang: Some(lang),
//         })
//     }
// }

#[async_trait]
impl<S> FromRequestParts<S> for WibbleRequest
where
    AppState: FromRef<S>,
    S: Sync + Send + 'static,
    WibbleRequest: Sync + Send + 'static,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let query = Query::<HashMap<String, String>>::try_from_uri(&parts.uri).ok();
        let lang = query
            .clone()
            .and_then(|q| q.get("lang").cloned())
            .or_else(|| {
                parts
                    .headers
                    .get("Accept-Language")
                    .and_then(|h| h.to_str().ok())
                    .map(String::from)
            })
            .unwrap_or("en-US".to_string());

        let style = query
            .and_then(|q| q.get("theme").cloned())
            .unwrap_or("style".to_string());
        let state = AppState::from_ref(state);
        Ok(WibbleRequest { state, style, lang })
    }
}
