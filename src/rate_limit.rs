use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use governor::{
    clock::DefaultClock,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use std::{
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};




// Define rate limiting quotas at construction time
// Global: 100 articles per hour (rolling window) with burst capacity 100
// Note: We build this quota in RateLimitState::new() to allow burst configuration.

// Shared state for rate limiters
#[derive(Clone, Debug)]
pub struct RateLimitState {
    pub global_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub global_rate_limit_hits: Arc<AtomicU64>,
    pub total_requests: Arc<AtomicU64>,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitState {
    pub fn new() -> Self {
        // Configure quotas with appropriate burst sizes to implement a rolling window
        let global_quota = Quota::per_hour(NonZeroU32::new(100).unwrap())
            .allow_burst(NonZeroU32::new(100).unwrap());
        Self {
            global_limiter: Arc::new(RateLimiter::new(global_quota, InMemoryState::default(), &DefaultClock::default())),
            global_rate_limit_hits: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
        }
    }


}



// Middleware function for rate limiting
pub async fn rate_limit_middleware(
    state: axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Increment total requests (for monitoring only)
    state.total_requests.fetch_add(1, Ordering::Relaxed);

    // Enforce only the global limit on POST /create
    if request.method() == axum::http::Method::POST && request.uri().path() == "/create" {
        if state.global_limiter.check().is_err() {
            state.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Global rate limit exceeded for article creation");
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    Ok(next.run(request).await)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_global_burst_allows_100_immediate() {
        let state = RateLimitState::new();
        for i in 0..100u32 {
            assert!(state.global_limiter.check().is_ok(), "failed at {}", i);
        }
        assert!(state.global_limiter.check().is_err(), "101st should fail");
    }


}