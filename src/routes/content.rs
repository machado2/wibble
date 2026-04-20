mod actions;
mod navigation;

use axum::extract::{Path, Query};
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;

use crate::app_state::AppState;
use crate::content as content_page;
use crate::error::Error;
use crate::services::article_language::{
    requested_article_language_query_value, resolve_article_language,
    resolve_requested_article_language,
};
use crate::wibble_request::WibbleRequest;

use self::actions::{post_comment, post_vote};
use self::navigation::{article_language_cookie_header, content_location_with_query};

#[cfg(test)]
use self::navigation::content_location;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/content/{slug}", get(get_content))
        .route("/content/{slug}/vote", post(post_vote))
        .route("/content/{slug}/comments", post(post_comment))
}

#[derive(Deserialize)]
struct ContentQuery {
    source: Option<String>,
    comments_page: Option<u64>,
    lang: Option<String>,
}

#[derive(Deserialize)]
struct PostCommentData {
    body: String,
    lang: Option<String>,
}

#[derive(Deserialize)]
struct VoteData {
    direction: String,
    lang: Option<String>,
}

async fn get_content(
    wr: WibbleRequest,
    Path(slug): Path<String>,
    Query(query): Query<ContentQuery>,
) -> Result<Response, Error> {
    let requested_language = resolve_requested_article_language(query.lang.as_deref());
    if let Some(raw_language) = query.lang.as_deref() {
        let canonical_language = requested_language.map(requested_article_language_query_value);
        if canonical_language != Some(raw_language) {
            return Ok(axum::response::Redirect::to(&content_location_with_query(
                &slug,
                query.source.as_deref(),
                query.comments_page,
                canonical_language,
                None,
            ))
            .into_response());
        }
    }
    let requested_language = requested_language.unwrap_or(None);
    let automatic_selection =
        resolve_article_language(None, None, wr.browser_translation_language, &[]);
    let cookie_header = article_language_cookie_header(
        &slug,
        requested_language,
        automatic_selection.preferred_language.code,
        query.lang.is_some(),
    );
    let mut content_request = wr.clone();
    if query.lang.is_some() && requested_language.is_none() {
        content_request.saved_article_language = None;
    }
    let response = content_page::GetContent::get_content(
        &content_request,
        &slug,
        query.source.as_deref(),
        query.comments_page,
        requested_language,
    )
    .await?;

    Ok(match cookie_header {
        Some(cookie) => ([(SET_COOKIE, cookie)], response).into_response(),
        None => response.into_response(),
    })
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::find_supported_translation_language;

    use super::{article_language_cookie_header, content_location, content_location_with_query};

    #[test]
    fn content_location_preserves_language_query() {
        assert_eq!(
            content_location("test-story", Some("pt"), None),
            "/content/test-story?lang=pt"
        );
    }

    #[test]
    fn content_location_appends_anchor_after_language_query() {
        assert_eq!(
            content_location("test-story", Some("pt"), Some("comments")),
            "/content/test-story?lang=pt#comments"
        );
    }

    #[test]
    fn content_location_with_query_preserves_existing_params() {
        assert_eq!(
            content_location_with_query(
                "test-story",
                Some("top"),
                Some(3),
                Some("pt"),
                Some("comments"),
            ),
            "/content/test-story?source=top&comments_page=3&lang=pt#comments"
        );
    }

    #[test]
    fn article_language_cookie_header_sets_manual_article_cookie() {
        let cookie = article_language_cookie_header(
            "test-story",
            find_supported_translation_language("pt"),
            "en",
            true,
        )
        .unwrap();

        assert!(cookie.contains("__article_lang=pt"));
        assert!(cookie.contains("Path=/content/test-story"));
    }

    #[test]
    fn article_language_cookie_header_clears_cookie_for_automatic_mode() {
        let cookie = article_language_cookie_header("test-story", None, "en", true).unwrap();

        assert!(cookie.contains("__article_lang="));
        assert!(cookie.contains("Max-Age=0"));
    }

    #[test]
    fn article_language_cookie_header_skips_cookie_when_no_query_was_provided() {
        assert!(article_language_cookie_header("test-story", None, "en", false).is_none());
    }

    #[test]
    fn article_language_cookie_header_clears_cookie_when_choice_matches_automatic_language() {
        let cookie = article_language_cookie_header(
            "test-story",
            find_supported_translation_language("pt"),
            "pt",
            true,
        )
        .unwrap();

        assert!(cookie.contains("Max-Age=0"));
    }
}
