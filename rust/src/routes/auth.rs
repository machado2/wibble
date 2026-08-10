use std::env;

use axum::extract::Query;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::get;
use axum::Router;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::distr::{Alphanumeric, SampleString};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::form_urlencoded::Serializer;

use crate::app_state::AppState;
use crate::auth::{sign_auth_user, AuthUser};
use crate::error::Error;
use crate::wibble_request::WibbleRequest;

pub fn localized_router() -> Router<AppState> {
    Router::new()
        .route("/login", get(login))
        .route("/logout", get(logout))
}

pub fn global_callback_router() -> Router<AppState> {
    Router::new().route("/auth/callback", get(auth_callback))
}

#[derive(Deserialize)]
struct AuthCallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    id_token: String,
    access_token: String,
}

#[derive(Deserialize)]
struct UserInfoResponse {
    sub: String,
    email: String,
    name: String,
    picture: Option<String>,
}

const OAUTH_STATE_COOKIE: &str = "__oauth_state";
const OAUTH_VERIFIER_COOKIE: &str = "__oauth_verifier";
const OAUTH_NONCE_COOKIE: &str = "__oauth_nonce";
const OAUTH_RETURN_COOKIE: &str = "__oauth_return";

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

fn transient_cookie(name: &str, value: &str, max_age: u64) -> String {
    let secure = if site_url_from_env().starts_with("https://") {
        "; Secure"
    } else {
        ""
    };
    format!(
        "{}={}; Path=/; HttpOnly; SameSite=Lax{}; Max-Age={}",
        name, value, secure, max_age
    )
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .map(str::trim)
        .find_map(|cookie| cookie.strip_prefix(&format!("{}=", name)))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn random_url_safe(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), length)
}

fn sso_issuer_url() -> String {
    env::var("SSO_ISSUER_URL")
        .unwrap_or_else(|_| "https://sso.fbmac.net/api/auth".to_string())
        .trim_end_matches('/')
        .to_string()
}

fn callback_url() -> String {
    format!("{}/auth/callback", site_url_from_env())
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
    headers: HeaderMap,
    Query(params): Query<AuthCallbackParams>,
) -> Result<Response, Error> {
    if let Some(error) = params.error {
        return Err(Error::Auth(format!("SSO authorization failed: {}", error)));
    }
    let code = params
        .code
        .ok_or_else(|| Error::Auth("Missing authorization code".to_string()))?;
    let state = params
        .state
        .ok_or_else(|| Error::Auth("Missing OAuth state".to_string()))?;
    let expected_state = cookie_value(&headers, OAUTH_STATE_COOKIE)
        .ok_or_else(|| Error::Auth("Missing OAuth state cookie".to_string()))?;
    if state != expected_state {
        return Err(Error::Auth("OAuth state does not match".to_string()));
    }
    let verifier = cookie_value(&headers, OAUTH_VERIFIER_COOKIE)
        .ok_or_else(|| Error::Auth("Missing PKCE verifier cookie".to_string()))?;
    let nonce = cookie_value(&headers, OAUTH_NONCE_COOKIE)
        .ok_or_else(|| Error::Auth("Missing OIDC nonce cookie".to_string()))?;

    let client_id = env::var("SSO_CLIENT_ID").expect("SSO_CLIENT_ID must be set");
    let client_secret = env::var("SSO_CLIENT_SECRET").expect("SSO_CLIENT_SECRET must be set");
    let token_response = reqwest::Client::new()
        .post(format!("{}/oauth2/token", sso_issuer_url()))
        .basic_auth(&client_id, Some(&client_secret))
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("redirect_uri", callback_url().as_str()),
            ("code_verifier", verifier.as_str()),
        ])
        .send()
        .await
        .map_err(|e| Error::Auth(format!("Failed to exchange authorization code: {}", e)))?;
    let status = token_response.status();
    if !status.is_success() {
        let body = token_response.text().await.unwrap_or_default();
        return Err(Error::Auth(format!(
            "Authorization code exchange failed with HTTP {}: {}",
            status, body
        )));
    }
    let tokens = token_response
        .json::<TokenResponse>()
        .await
        .map_err(|e| Error::Auth(format!("Failed to parse token response: {}", e)))?;
    let subject = wr
        .state
        .jwks_client
        .validate_token_with_nonce(&tokens.id_token, Some(&nonce))
        .await?;
    let userinfo_response = reqwest::Client::new()
        .get(format!("{}/oauth2/userinfo", sso_issuer_url()))
        .bearer_auth(&tokens.access_token)
        .send()
        .await
        .map_err(|e| Error::Auth(format!("Failed to fetch OIDC UserInfo: {}", e)))?;
    let status = userinfo_response.status();
    if !status.is_success() {
        return Err(Error::Auth(format!(
            "OIDC UserInfo failed with HTTP {}",
            status
        )));
    }
    let profile = userinfo_response
        .json::<UserInfoResponse>()
        .await
        .map_err(|e| Error::Auth(format!("Failed to parse OIDC UserInfo: {}", e)))?;
    if profile.sub != subject {
        return Err(Error::Auth(
            "OIDC UserInfo subject does not match ID token".to_string(),
        ));
    }
    let user = AuthUser {
        sub: profile.sub,
        email: profile.email,
        name: profile.name,
        picture: profile.picture,
    };
    let signed_profile = sign_auth_user(&user)?;
    let redirect_url = cookie_value(&headers, OAUTH_RETURN_COOKIE)
        .and_then(|target| sanitize_redirect_target(Some(target)))
        .unwrap_or_else(|| wr.localized_root_path());

    let mut response = Redirect::to(&redirect_url).into_response();
    for cookie in [
        auth_cookie(&tokens.id_token, 60 * 60),
        transient_cookie("__auth_profile", &signed_profile, 60 * 60),
        transient_cookie(OAUTH_STATE_COOKIE, "", 0),
        transient_cookie(OAUTH_VERIFIER_COOKIE, "", 0),
        transient_cookie(OAUTH_NONCE_COOKIE, "", 0),
        transient_cookie(OAUTH_RETURN_COOKIE, "", 0),
    ] {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    Ok(response)
}

async fn login(wr: WibbleRequest) -> Response {
    let state = random_url_safe(40);
    let verifier = random_url_safe(64);
    let nonce = random_url_safe(40);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let client_id = env::var("SSO_CLIENT_ID").expect("SSO_CLIENT_ID must be set");
    let query = Serializer::new(String::new())
        .append_pair("client_id", &client_id)
        .append_pair("redirect_uri", &callback_url())
        .append_pair("response_type", "code")
        .append_pair("scope", "openid profile email")
        .append_pair("state", &state)
        .append_pair("nonce", &nonce)
        .append_pair("code_challenge", &challenge)
        .append_pair("code_challenge_method", "S256")
        .finish();
    let mut response =
        Redirect::to(&format!("{}/oauth2/authorize?{}", sso_issuer_url(), query)).into_response();
    for cookie in [
        transient_cookie(OAUTH_STATE_COOKIE, &state, 600),
        transient_cookie(OAUTH_VERIFIER_COOKIE, &verifier, 600),
        transient_cookie(OAUTH_NONCE_COOKIE, &nonce, 600),
        transient_cookie(OAUTH_RETURN_COOKIE, &wr.localized_root_path(), 600),
    ] {
        if let Ok(value) = HeaderValue::from_str(&cookie) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    response
}

async fn logout(wr: WibbleRequest, headers: HeaderMap) -> Response {
    let our_url = format!("{}/", site_url_from_env());
    let cookie = auth_cookie("", 0);
    let profile_cookie = transient_cookie("__auth_profile", "", 0);
    let redirect = if let Some(id_token) = cookie_value(&headers, "__auth") {
        let query = Serializer::new(String::new())
            .append_pair("id_token_hint", &id_token)
            .append_pair("post_logout_redirect_uri", &our_url)
            .finish();
        Redirect::to(&format!(
            "{}/oauth2/end-session?{}",
            sso_issuer_url(),
            query
        ))
    } else {
        Redirect::to(&wr.localized_root_path())
    };
    let mut response = redirect.into_response();
    for value in [cookie, profile_cookie] {
        if let Ok(value) = HeaderValue::from_str(&value) {
            response.headers_mut().append(SET_COOKIE, value);
        }
    }
    response
}
