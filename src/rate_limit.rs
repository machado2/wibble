use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
};
use governor::{
    clock::DefaultClock,
    state::keyed::DefaultKeyedStateStore,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{
    collections::HashMap,
    net::IpAddr,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};
use tokio::sync::RwLock;

type IpRateLimiter = Arc<RateLimiter<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>>;

// Define rate limiting quotas
// Global: 100 articles per hour
const GLOBAL_QUOTA: Quota = Quota::per_hour(NonZeroU32::new(100).unwrap());
// Per-IP: 5 requests per hour
const PER_IP_QUOTA: Quota = Quota::per_hour(NonZeroU32::new(5).unwrap());

// Shared state for rate limiters
#[derive(Clone, Debug)]
pub struct RateLimitState {
    pub global_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub per_ip_limiter: Arc<RwLock<HashMap<IpAddr, IpRateLimiter>>>,
    // Monitoring counters
    pub global_rate_limit_hits: Arc<AtomicU64>,
    pub per_ip_rate_limit_hits: Arc<AtomicU64>,
    // Cost tracking placeholder (e.g., total requests processed)
    pub total_requests: Arc<AtomicU64>,
    // Concurrent job limits per IP (max 2 active jobs per IP)
    pub active_jobs_per_ip: Arc<RwLock<HashMap<IpAddr, u32>>>,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitState {
    pub fn new() -> Self {
        Self {
            global_limiter: Arc::new(RateLimiter::new(GLOBAL_QUOTA, InMemoryState::default(), &DefaultClock::default())),
            per_ip_limiter: Arc::new(RwLock::new(HashMap::new())),
            global_rate_limit_hits: Arc::new(AtomicU64::new(0)),
            per_ip_rate_limit_hits: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
            active_jobs_per_ip: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_or_create_ip_limiter(&self, ip: IpAddr) -> IpRateLimiter {
        let mut map = self.per_ip_limiter.write().await;
        map.entry(ip).or_insert_with(|| {
            Arc::new(RateLimiter::new(PER_IP_QUOTA, DefaultKeyedStateStore::default(), &DefaultClock::default()))
        }).clone()
    }

    // Check and increment active jobs for IP
    pub async fn can_start_job(&self, ip: Option<IpAddr>) -> bool {
        if let Some(ip) = ip {
            let mut map = self.active_jobs_per_ip.write().await;
            let count = map.entry(ip).or_insert(0);
            if *count < 2 {
                *count += 1;
                true
            } else {
                false
            }
        } else {
            // If no IP, allow (but this is less secure)
            true
        }
    }

    // Decrement active jobs when job completes
    pub async fn job_completed(&self, ip: Option<IpAddr>) {
        if let Some(ip) = ip {
            let mut map = self.active_jobs_per_ip.write().await;
            if let Some(count) = map.get_mut(&ip) {
                if *count > 0 {
                    *count -= 1;
                }
            }
        }
    }
}

// Function to extract real IP from headers, handling proxies
pub fn extract_real_ip(headers: &HeaderMap) -> Option<IpAddr> {
    // Check X-Forwarded-For first (common in proxies)
    if let Some(x_forwarded_for) = headers.get("x-forwarded-for") {
        if let Ok(value) = x_forwarded_for.to_str() {
            // X-Forwarded-For can have multiple IPs, take the first (original client)
            if let Some(first_ip) = value.split(',').next() {
                if let Ok(ip) = first_ip.trim().parse::<IpAddr>() {
                    return Some(ip);
                }
            }
        }
    }

    // Fallback to X-Real-IP
    if let Some(x_real_ip) = headers.get("x-real-ip") {
        if let Ok(value) = x_real_ip.to_str() {
            if let Ok(ip) = value.parse::<IpAddr>() {
                return Some(ip);
            }
        }
    }

    // If no headers, return None (for cases where IP is not available)
    None
}

// Middleware function for rate limiting
pub async fn rate_limit_middleware(
    state: axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let headers = request.headers();

    // Increment total requests
    state.total_requests.fetch_add(1, Ordering::Relaxed);

    let ip = extract_real_ip(headers);

    // Check global rate limit only for article creation (POST /create)
    if request.method() == axum::http::Method::POST && request.uri().path() == "/create"
        && state.global_limiter.check_n(NonZeroU32::new(1).unwrap()).is_err() {
        // Increment global rate limit hits
        state.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
        // Log global rate limit hit
        tracing::warn!("Global rate limit exceeded for article creation");
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Extract IP for per-IP limiting
    if let Some(ip) = ip {
        let ip_limiter = state.get_or_create_ip_limiter(ip).await;
        if ip_limiter.check_key_n(&ip, NonZeroU32::new(1).unwrap()).is_err() {
            // Increment per-IP rate limit hits
            state.per_ip_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            // Log per-IP rate limit hit
            tracing::warn!("Per-IP rate limit exceeded for IP: {}", ip);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }

        // Check concurrent jobs for creation endpoints (assume /create)
        // In a real app, check the path
        if request.uri().path().starts_with("/create") && !state.can_start_job(Some(ip)).await {
            tracing::warn!("Concurrent job limit exceeded for IP: {}", ip);
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    } else {
        // If no IP available, perhaps allow or log
        tracing::warn!("Unable to extract real IP for rate limiting");
        // For security, perhaps deny or use a fallback
        // For now, allow to avoid blocking legitimate requests behind proxies
    }

    // Proceed with the request
    Ok(next.run(request).await)
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    #[tokio::test]
    async fn test_extract_real_ip() {
        let mut headers = HeaderMap::new();
        headers.insert("x-forwarded-for", "192.168.1.1, 10.0.0.1".parse().unwrap());
        assert_eq!(extract_real_ip(&headers), Some("192.168.1.1".parse::<IpAddr>().unwrap()));

        let mut headers2 = HeaderMap::new();
        headers2.insert("x-real-ip", "10.0.0.1".parse().unwrap());
        assert_eq!(extract_real_ip(&headers2), Some("10.0.0.1".parse::<IpAddr>().unwrap()));

        let headers3 = HeaderMap::new();
        assert_eq!(extract_real_ip(&headers3), None);
    }

    #[tokio::test]
    async fn test_rate_limit_state() {
        let state = RateLimitState::new();
        let ip: IpAddr = "127.0.0.1".parse().unwrap();

        // Should allow starting job
        assert!(state.can_start_job(Some(ip)).await);
        assert!(state.can_start_job(Some(ip)).await);
        // Third should fail
        assert!(!state.can_start_job(Some(ip)).await);

        // Complete one
        state.job_completed(Some(ip)).await;
        // Now allow again
        assert!(state.can_start_job(Some(ip)).await);
    }
}