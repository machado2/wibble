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
    env,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};




// Shared state for rate limiters
#[derive(Clone, Debug)]
pub struct RateLimitState {
    pub hourly_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
    pub daily_limiter: Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>,
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
        let max_per_hour: u32 = env::var("MAX_ARTICLES_PER_HOUR")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        let max_per_day: u32 = env::var("MAX_ARTICLES_PER_DAY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);

        let hourly_quota = Quota::per_hour(NonZeroU32::new(max_per_hour).expect("MAX_ARTICLES_PER_HOUR must be > 0"))
            .allow_burst(NonZeroU32::new(max_per_hour).unwrap());
        let daily_quota = Quota::with_period(std::time::Duration::from_secs(86400 / max_per_day as u64))
            .expect("MAX_ARTICLES_PER_DAY must be > 0")
            .allow_burst(NonZeroU32::new(max_per_day).unwrap());

        Self {
            hourly_limiter: Arc::new(RateLimiter::new(hourly_quota, InMemoryState::default(), &DefaultClock::default())),
            daily_limiter: Arc::new(RateLimiter::new(daily_quota, InMemoryState::default(), &DefaultClock::default())),
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

    // Enforce hourly and daily limits on POST /create
    if request.method() == axum::http::Method::POST && request.uri().path() == "/create" {
        if state.hourly_limiter.check().is_err() {
            state.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Hourly rate limit exceeded for article creation");
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
        if state.daily_limiter.check().is_err() {
            state.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Daily rate limit exceeded for article creation");
            return Err(StatusCode::TOO_MANY_REQUESTS);
        }
    }

    Ok(next.run(request).await)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hourly_burst_allows_default_and_blocks_next() {
        let state = RateLimitState::new();
        let max: u32 = env::var("MAX_ARTICLES_PER_HOUR")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        for i in 0..max {
            assert!(state.hourly_limiter.check().is_ok(), "failed at {}", i);
        }
        assert!(state.hourly_limiter.check().is_err(), "should fail after max");
    }

    #[tokio::test]
    async fn test_daily_burst_allows_default_and_blocks_next() {
        let state = RateLimitState::new();
        let max: u32 = env::var("MAX_ARTICLES_PER_DAY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(20);
        for i in 0..max {
            assert!(state.daily_limiter.check().is_ok(), "failed at {}", i);
        }
        assert!(state.daily_limiter.check().is_err(), "should fail after max");
    }
}