use std::env;

use axum::extract::Query;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use serde::Deserialize;
use url::form_urlencoded::Serializer;

use crate::app_state::AppState;
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub fn localized_router() -> Router<AppState> {
    Router::new()
        .route("/auth/callback", get(auth_callback))
        .route("/login", get(login))
        .route("/logout", get(logout))
}

pub fn global_callback_router() -> Router<AppState> {
    Router::new().route("/auth/callback", get(auth_callback))
}

#[derive(Deserialize)]
struct AuthCallbackParams {
    token: Option<String>,
    redirect: Option<String>,
}

fn site_url_from_env() -> String {
    env::var("SITE_URL")
        .unwrap_or_else(|_| "http://localhost:8000".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn auth_cookie(token: &str, max_age: u64) -> String {
    let secure = if site_url_from_env().starts_with("https://") {
        "; Secure"
    } else {
        ""
    };

    format!(
        "__auth={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={}",
        token, secure, max_age
    )
}

fn sanitize_redirect_target(raw: Option<String>) -> Option<String> {
    let site_url = site_url_from_env();

    raw.and_then(|target| {
        if target.starts_with('/') && !target.starts_with("//") {
            Some(target)
        } else if target.starts_with(&site_url) {
            let relative = target[site_url.len()..].to_string();
            if relative.starts_with('/') {
                Some(relative)
            } else {
                Some("/".to_string())
            }
        } else {
            None
        }
    })
}

async fn auth_callback(
    wr: WibbleRequest,
    Query(params): Query<AuthCallbackParams>,
) -> Result<Response, Error> {
    let token = params
        .token
        .ok_or_else(|| Error::Auth("Missing token".to_string()))?;
    let _user = wr.state.jwks_client.validate_token(&token).await?;
    let cookie = auth_cookie(&token, 30 * 24 * 60 * 60);
    let redirect_url =
        sanitize_redirect_target(params.redirect).unwrap_or_else(|| wr.localized_root_path());

    Ok(([(SET_COOKIE, cookie)], Redirect::to(&redirect_url)).into_response())
}

async fn login(wr: WibbleRequest) -> Redirect {
    let auth_url =
        env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "https://auth.fbmac.net".to_string());
    let callback_url = format!(
        "{}{}",
        site_url_from_env().trim_end_matches('/'),
        wr.localized_path("/auth/callback")
    );
    let query = Serializer::new(String::new())
        .append_pair("redirect", &callback_url)
        .append_pair("mode", "both")
        .finish();

    Redirect::to(&format!("{}/login?{}", auth_url, query))
}

async fn logout(wr: WibbleRequest) -> Response {
    let auth_url =
        env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "https://auth.fbmac.net".to_string());
    let our_url = format!("{}{}", site_url_from_env(), wr.localized_root_path());
    let cookie = auth_cookie("", 0);
    let query = Serializer::new(String::new())
        .append_pair("redirect", &our_url)
        .finish();

    (
        [(SET_COOKIE, cookie)],
        Redirect::to(&format!("{}/logout?{}", auth_url, query)),
    )
        .into_response()
}
