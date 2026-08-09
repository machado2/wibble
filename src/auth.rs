use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use jsonwebtoken::jwk::{Jwk, JwkSet};
use jsonwebtoken::{decode, decode_header, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::sync::RwLock;

use crate::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub sub: String,
    pub email: String,
    pub name: String,
    pub picture: Option<String>,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        let admin_email = env::var("ADMIN_EMAIL").unwrap_or_default();
        self.email == admin_email
    }
}

#[derive(Debug, Deserialize)]
struct JwtClaims {
    #[allow(dead_code)]
    iss: String,
    sub: String,
    iat: u64,
    exp: Option<u64>,
    nonce: Option<String>,
}

#[derive(Debug)]
struct CachedJwks {
    keys: Vec<Jwk>,
    fetched_at: Instant,
}

#[derive(Clone, Debug)]
pub struct JwksClient {
    cache: Arc<RwLock<Option<CachedJwks>>>,
    issuer_url: String,
    jwks_url: String,
    client_id: String,
    http: reqwest::Client,
}

impl Default for JwksClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JwksClient {
    pub fn new() -> Self {
        let issuer_url = env::var("SSO_ISSUER_URL")
            .unwrap_or_else(|_| "https://sso.fbmac.net/api/auth".to_string())
            .trim_end_matches('/')
            .to_string();
        let jwks_url = env::var("SSO_JWKS_URL").unwrap_or_else(|_| format!("{}/jwks", issuer_url));
        let client_id = env::var("SSO_CLIENT_ID").expect("SSO_CLIENT_ID must be set");
        Self {
            cache: Arc::new(RwLock::new(None)),
            issuer_url,
            jwks_url,
            client_id,
            http: reqwest::Client::new(),
        }
    }

    async fn fetch_jwks(&self) -> Result<Vec<Jwk>, Error> {
        {
            let cache = self.cache.read().await;
            if let Some(cached) = &*cache {
                if cached.fetched_at.elapsed() < Duration::from_secs(86400) {
                    return Ok(cached.keys.clone());
                }
            }
        }

        let response = self
            .http
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Failed to fetch JWKS: {}", e)))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::Auth(format!(
                "Failed to fetch JWKS: HTTP {}",
                status
            )));
        }
        let body = response
            .text()
            .await
            .map_err(|e| Error::Auth(format!("Failed to read JWKS response: {}", e)))?;
        let resp: JwkSet = serde_json::from_str(&body)
            .map_err(|e| Error::Auth(format!("Failed to parse JWKS JSON: {}", e)))?;

        let keys = resp.keys;
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedJwks {
                keys: keys.clone(),
                fetched_at: Instant::now(),
            });
        }
        Ok(keys)
    }

    pub async fn validate_token(&self, token: &str) -> Result<String, Error> {
        self.validate_token_with_nonce(token, None).await
    }

    pub async fn validate_token_with_nonce(
        &self,
        token: &str,
        expected_nonce: Option<&str>,
    ) -> Result<String, Error> {
        let header = decode_header(token)
            .map_err(|e| Error::Auth(format!("Failed to decode JWT header: {}", e)))?;
        let kid = header
            .kid
            .ok_or_else(|| Error::Auth("Missing kid in JWT header".to_string()))?;

        let keys = self.fetch_jwks().await?;
        let jwk = keys
            .into_iter()
            .find(|key| key.common.key_id.as_deref() == Some(&kid))
            .ok_or_else(|| Error::Auth(format!("Key {} not found in JWKS", kid)))?;

        let decoding_key = DecodingKey::from_jwk(&jwk)
            .map_err(|e| Error::Auth(format!("Failed to create decoding key from JWK: {}", e)))?;

        let mut validation = Validation::new(header.alg);
        validation.validate_exp = false;
        validation.set_required_spec_claims::<&str>(&[]);
        validation.set_issuer(&[&self.issuer_url]);
        validation.set_audience(&[&self.client_id]);

        let token_data = decode::<JwtClaims>(token, &decoding_key, &validation)
            .map_err(|e| Error::Auth(format!("JWT validation failed: {}", e)))?;

        let max_age_secs: u64 = env::var("AUTH_MAX_AGE_SECONDS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(7 * 24 * 60 * 60);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if let Some(exp) = token_data.claims.exp {
            if now >= exp {
                return Err(Error::Auth("Token has expired".to_string()));
            }
        }
        if now.saturating_sub(token_data.claims.iat) > max_age_secs {
            return Err(Error::Auth("Token is too old".to_string()));
        }
        if let Some(expected_nonce) = expected_nonce {
            if token_data.claims.nonce.as_deref() != Some(expected_nonce) {
                return Err(Error::Auth("OIDC nonce does not match".to_string()));
            }
        }

        Ok(token_data.claims.sub)
    }
}

fn session_secret() -> Result<String, Error> {
    env::var("AUTH_SESSION_SECRET")
        .or_else(|_| env::var("SSO_CLIENT_SECRET"))
        .map_err(|_| {
            Error::Auth("AUTH_SESSION_SECRET or SSO_CLIENT_SECRET must be set".to_string())
        })
}

pub fn sign_auth_user(user: &AuthUser) -> Result<String, Error> {
    let payload = serde_json::to_vec(user)
        .map_err(|e| Error::Auth(format!("Failed to serialize auth profile: {}", e)))?;
    let payload = URL_SAFE_NO_PAD.encode(payload);
    let mut mac = Hmac::<Sha256>::new_from_slice(session_secret()?.as_bytes())
        .map_err(|e| Error::Auth(format!("Failed to initialize session signature: {}", e)))?;
    mac.update(payload.as_bytes());
    let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());
    Ok(format!("{}.{}", payload, signature))
}

pub fn verify_auth_user(value: &str, expected_sub: &str) -> Result<AuthUser, Error> {
    let (payload, signature) = value
        .split_once('.')
        .ok_or_else(|| Error::Auth("Malformed auth profile cookie".to_string()))?;
    let signature = URL_SAFE_NO_PAD
        .decode(signature)
        .map_err(|_| Error::Auth("Malformed auth profile signature".to_string()))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(session_secret()?.as_bytes())
        .map_err(|e| Error::Auth(format!("Failed to initialize session signature: {}", e)))?;
    mac.update(payload.as_bytes());
    mac.verify_slice(&signature)
        .map_err(|_| Error::Auth("Invalid auth profile signature".to_string()))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|_| Error::Auth("Malformed auth profile payload".to_string()))?;
    let user: AuthUser = serde_json::from_slice(&bytes)
        .map_err(|_| Error::Auth("Malformed auth profile JSON".to_string()))?;
    if user.sub != expected_sub {
        return Err(Error::Auth(
            "Auth profile subject does not match ID token".to_string(),
        ));
    }
    Ok(user)
}

pub fn extract_cookie(parts: &http::request::Parts, name: &str) -> Option<String> {
    let cookie_header = parts.headers.get("cookie")?.to_str().ok()?;
    cookie_header.split(';').map(str::trim).find_map(|cookie| {
        cookie
            .strip_prefix(&format!("{}=", name))
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

pub fn extract_auth_token(parts: &http::request::Parts) -> Option<String> {
    if let Some(cookie_header) = parts.headers.get("cookie") {
        if let Ok(cookie_str) = cookie_header.to_str() {
            for cookie in cookie_str.split(';') {
                let cookie = cookie.trim();
                if let Some(value) = cookie.strip_prefix("__auth=") {
                    if !value.is_empty() {
                        return Some(value.to_string());
                    }
                }
            }
        }
    }

    if let Some(auth_header) = parts.headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(token) = auth_str.strip_prefix("Bearer ") {
                if !token.is_empty() {
                    return Some(token.to_string());
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{sign_auth_user, verify_auth_user, AuthUser};
    use jsonwebtoken::jwk::JwkSet;
    use jsonwebtoken::DecodingKey;

    #[test]
    fn accepts_ed25519_jwks_published_by_sso() {
        let jwks: JwkSet = serde_json::from_str(
            r#"{"keys":[{"alg":"EdDSA","crv":"Ed25519","kty":"OKP","x":"LPf82VxFeQV9px1xbtwHYFw7uy9PuTRG0ymbfDBkFuw","kid":"test-key"}]}"#,
        )
        .unwrap();

        assert_eq!(jwks.keys.len(), 1);
        DecodingKey::from_jwk(&jwks.keys[0]).unwrap();
    }

    #[test]
    fn signed_auth_profile_rejects_tampering_and_subject_mismatch() {
        std::env::set_var("AUTH_SESSION_SECRET", "unit-test-secret");
        let user = AuthUser {
            sub: "user-1".into(),
            email: "test@example.com".into(),
            name: "Test".into(),
            picture: None,
        };
        let signed = sign_auth_user(&user).unwrap();
        assert_eq!(
            verify_auth_user(&signed, "user-1").unwrap().email,
            user.email
        );
        assert!(verify_auth_user(&signed, "user-2").is_err());
        let mut tampered = signed.into_bytes();
        tampered[0] = if tampered[0] == b'a' { b'b' } else { b'a' };
        assert!(verify_auth_user(std::str::from_utf8(&tampered).unwrap(), "user-1").is_err());
    }
}
