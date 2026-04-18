use std::env;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
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
    email: String,
    name: String,
    picture: Option<String>,
}

impl AuthUser {
    fn from_claims(claims: JwtClaims) -> Self {
        Self {
            sub: claims.sub,
            email: claims.email,
            name: claims.name,
            picture: claims.picture,
        }
    }
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize, Clone)]
struct Jwk {
    kid: String,
    #[allow(dead_code)]
    kty: String,
    n: String,
    e: String,
}

#[derive(Debug)]
struct CachedJwks {
    keys: Vec<Jwk>,
    fetched_at: Instant,
}

#[derive(Clone, Debug)]
pub struct JwksClient {
    cache: Arc<RwLock<Option<CachedJwks>>>,
    auth_service_url: String,
    http: reqwest::Client,
}

impl Default for JwksClient {
    fn default() -> Self {
        Self::new()
    }
}

impl JwksClient {
    pub fn new() -> Self {
        let auth_service_url =
            env::var("AUTH_SERVICE_URL").unwrap_or_else(|_| "https://auth.fbmac.net".to_string());
        Self {
            cache: Arc::new(RwLock::new(None)),
            auth_service_url,
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

        let url = format!("{}/.well-known/jwks.json", self.auth_service_url);
        let resp: JwksResponse = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Auth(format!("Failed to fetch JWKS: {}", e)))?
            .json()
            .await
            .map_err(|e| Error::Auth(format!("Failed to parse JWKS: {}", e)))?;

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

    pub async fn validate_token(&self, token: &str) -> Result<AuthUser, Error> {
        let header = decode_header(token)
            .map_err(|e| Error::Auth(format!("Failed to decode JWT header: {}", e)))?;
        let kid = header
            .kid
            .ok_or_else(|| Error::Auth("Missing kid in JWT header".to_string()))?;

        let keys = self.fetch_jwks().await?;
        let jwk = keys
            .into_iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| Error::Auth(format!("Key {} not found in JWKS", kid)))?;

        let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
            .map_err(|e| Error::Auth(format!("Failed to create decoding key: {}", e)))?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.validate_exp = false;
        validation.set_required_spec_claims::<&str>(&[]);
        validation.set_issuer(&[&self.auth_service_url]);

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

        Ok(AuthUser::from_claims(token_data.claims))
    }
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
