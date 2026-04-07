use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArticleRateLimit {
    Hourly,
    Daily,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitState {
    fn read_limit(var_name: &str, default: u32) -> u32 {
        env::var(var_name)
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(default)
    }

    fn read_burst(var_name: &str, max: u32) -> u32 {
        env::var(var_name)
            .ok()
            .and_then(|s| s.parse().ok())
            .map(|v: u32| v.clamp(1, max))
            .unwrap_or(max)
    }

    pub fn new() -> Self {
        let max_per_hour = Self::read_limit("MAX_ARTICLES_PER_HOUR", 20);
        let max_per_day = Self::read_limit("MAX_ARTICLES_PER_DAY", 20);
        let hourly_burst = Self::read_burst("MAX_ARTICLES_BURST_PER_HOUR", max_per_hour);
        let daily_burst = Self::read_burst("MAX_ARTICLES_BURST_PER_DAY", max_per_day);

        let hourly_quota = Quota::per_hour(
            NonZeroU32::new(max_per_hour).expect("MAX_ARTICLES_PER_HOUR must be > 0"),
        )
        .allow_burst(NonZeroU32::new(hourly_burst).unwrap());
        let daily_quota =
            Quota::with_period(std::time::Duration::from_secs(86400 / max_per_day as u64))
                .expect("MAX_ARTICLES_PER_DAY must be > 0")
                .allow_burst(NonZeroU32::new(daily_burst).unwrap());

        Self {
            hourly_limiter: Arc::new(RateLimiter::new(
                hourly_quota,
                InMemoryState::default(),
                &DefaultClock::default(),
            )),
            daily_limiter: Arc::new(RateLimiter::new(
                daily_quota,
                InMemoryState::default(),
                &DefaultClock::default(),
            )),
            global_rate_limit_hits: Arc::new(AtomicU64::new(0)),
            total_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn check_article_generation_limit(&self) -> Result<(), ArticleRateLimit> {
        if self.hourly_limiter.check().is_err() {
            self.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Hourly rate limit exceeded for article creation");
            return Err(ArticleRateLimit::Hourly);
        }
        if self.daily_limiter.check().is_err() {
            self.global_rate_limit_hits.fetch_add(1, Ordering::Relaxed);
            tracing::warn!("Daily rate limit exceeded for article creation");
            return Err(ArticleRateLimit::Daily);
        }

        Ok(())
    }
}

// Middleware function for request monitoring
pub async fn rate_limit_middleware(
    state: axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Increment total requests (for monitoring only)
    state.total_requests.fetch_add(1, Ordering::Relaxed);

    Ok(next.run(request).await)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hourly_burst_allows_default_and_blocks_next() {
        let state = RateLimitState::new();
        let max = RateLimitState::read_limit("MAX_ARTICLES_PER_HOUR", 20);
        let burst = RateLimitState::read_burst("MAX_ARTICLES_BURST_PER_HOUR", max);
        for i in 0..burst {
            assert!(state.hourly_limiter.check().is_ok(), "failed at {}", i);
        }
        assert!(
            state.hourly_limiter.check().is_err(),
            "should fail after max"
        );
    }

    #[tokio::test]
    async fn test_daily_burst_allows_default_and_blocks_next() {
        let state = RateLimitState::new();
        let max = RateLimitState::read_limit("MAX_ARTICLES_PER_DAY", 20);
        let burst = RateLimitState::read_burst("MAX_ARTICLES_BURST_PER_DAY", max);
        for i in 0..burst {
            assert!(state.daily_limiter.check().is_ok(), "failed at {}", i);
        }
        assert!(
            state.daily_limiter.check().is_err(),
            "should fail after max"
        );
    }
}
