use axum::{extract::Request, http::StatusCode, middleware::Next, response::Response};
use governor::{clock::DefaultClock, state::keyed::DefaultKeyedStateStore, Quota, RateLimiter};
use std::{
    collections::HashMap,
    env,
    num::NonZeroU32,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

type KeyedLimiter = RateLimiter<String, DefaultKeyedStateStore<String>, DefaultClock>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArticleRateLimit {
    Hourly,
    Daily,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationRateLimit {
    Hourly,
    Daily,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RequesterTier {
    Anonymous,
    Authenticated,
    Admin,
}

impl RequesterTier {
    fn env_suffix(self) -> &'static str {
        match self {
            Self::Anonymous => "ANON",
            Self::Authenticated => "AUTH",
            Self::Admin => "ADMIN",
        }
    }

    pub fn queue_priority_boost(self) -> i32 {
        match self {
            Self::Anonymous => 0,
            Self::Authenticated => 100,
            Self::Admin => 200,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum RateLimitCapability {
    PlainArticleGeneration,
    ResearchGeneration,
    EditAgentRequest,
    BackgroundTranslation,
    ImageRegeneration,
    ClarifyingQuestion,
}

impl RateLimitCapability {
    fn env_prefix(self) -> &'static str {
        match self {
            Self::PlainArticleGeneration => "MAX_ARTICLES",
            Self::ResearchGeneration => "MAX_RESEARCH_ARTICLES",
            Self::EditAgentRequest => "MAX_EDIT_AGENT_REQUESTS",
            Self::BackgroundTranslation => "MAX_TRANSLATIONS",
            Self::ImageRegeneration => "MAX_IMAGE_REGENERATIONS",
            Self::ClarifyingQuestion => "MAX_CLARIFYING_QUESTIONS",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::PlainArticleGeneration => "plain_article_generation",
            Self::ResearchGeneration => "research_generation",
            Self::EditAgentRequest => "edit_agent_request",
            Self::BackgroundTranslation => "background_translation",
            Self::ImageRegeneration => "image_regeneration",
            Self::ClarifyingQuestion => "clarifying_question",
        }
    }

    fn default_limits(self, tier: RequesterTier) -> CapabilityDefaults {
        match self {
            Self::PlainArticleGeneration => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(10, 20),
                RequesterTier::Authenticated => CapabilityDefaults::new(20, 40),
                RequesterTier::Admin => CapabilityDefaults::new(100, 200),
            },
            Self::ResearchGeneration => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(1, 2),
                RequesterTier::Authenticated => CapabilityDefaults::new(5, 10),
                RequesterTier::Admin => CapabilityDefaults::new(20, 50),
            },
            Self::EditAgentRequest => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(1, 2),
                RequesterTier::Authenticated => CapabilityDefaults::new(10, 20),
                RequesterTier::Admin => CapabilityDefaults::new(40, 80),
            },
            Self::BackgroundTranslation => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(20, 50),
                RequesterTier::Authenticated => CapabilityDefaults::new(40, 100),
                RequesterTier::Admin => CapabilityDefaults::new(200, 500),
            },
            Self::ImageRegeneration => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(1, 2),
                RequesterTier::Authenticated => CapabilityDefaults::new(10, 20),
                RequesterTier::Admin => CapabilityDefaults::new(50, 100),
            },
            Self::ClarifyingQuestion => match tier {
                RequesterTier::Anonymous => CapabilityDefaults::new(1, 2),
                RequesterTier::Authenticated => CapabilityDefaults::new(10, 20),
                RequesterTier::Admin => CapabilityDefaults::new(50, 100),
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum LimitWindow {
    Hourly,
    Daily,
}

impl LimitWindow {
    fn env_fragment(self) -> &'static str {
        match self {
            Self::Hourly => "PER_HOUR",
            Self::Daily => "PER_DAY",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct LimiterKey {
    capability: RateLimitCapability,
    tier: RequesterTier,
    window: LimitWindow,
}

#[derive(Clone, Copy, Debug)]
struct CapabilityDefaults {
    hourly: u32,
    daily: u32,
}

impl CapabilityDefaults {
    const fn new(hourly: u32, daily: u32) -> Self {
        Self { hourly, daily }
    }
}

// Shared state for rate limiters
#[derive(Clone, Debug)]
pub struct RateLimitState {
    limiters: Arc<HashMap<LimiterKey, Arc<KeyedLimiter>>>,
    hit_counters: Arc<HashMap<RateLimitCapability, Arc<AtomicU64>>>,
    pub total_requests: Arc<AtomicU64>,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self::new()
    }
}

impl RateLimitState {
    fn limiter_key(
        capability: RateLimitCapability,
        tier: RequesterTier,
        window: LimitWindow,
    ) -> LimiterKey {
        LimiterKey {
            capability,
            tier,
            window,
        }
    }

    fn capability_limit(
        capability: RateLimitCapability,
        tier: RequesterTier,
        window: LimitWindow,
    ) -> u32 {
        let defaults = capability.default_limits(tier);
        let default = match window {
            LimitWindow::Hourly => defaults.hourly,
            LimitWindow::Daily => defaults.daily,
        };
        let env_name = format!(
            "{}_{}_{}",
            capability.env_prefix(),
            window.env_fragment(),
            tier.env_suffix()
        );
        let fallback_env_name = format!("{}_{}", capability.env_prefix(), window.env_fragment());
        env::var(&env_name)
            .ok()
            .and_then(|value| value.parse().ok())
            .or_else(|| {
                env::var(&fallback_env_name)
                    .ok()
                    .and_then(|value| value.parse().ok())
            })
            .unwrap_or(default)
    }

    fn capability_burst(
        capability: RateLimitCapability,
        tier: RequesterTier,
        window: LimitWindow,
        max: u32,
    ) -> u32 {
        let env_name = format!(
            "{}_BURST_{}_{}",
            capability.env_prefix(),
            window.env_fragment(),
            tier.env_suffix()
        );
        let fallback_env_name = format!(
            "{}_BURST_{}",
            capability.env_prefix(),
            window.env_fragment()
        );
        env::var(&env_name)
            .ok()
            .and_then(|value| value.parse().ok())
            .or_else(|| {
                env::var(&fallback_env_name)
                    .ok()
                    .and_then(|value| value.parse().ok())
            })
            .map(|value: u32| value.clamp(1, max))
            .unwrap_or(max)
    }

    fn quota_for(
        capability: RateLimitCapability,
        tier: RequesterTier,
        window: LimitWindow,
    ) -> Quota {
        let max = Self::capability_limit(capability, tier, window);
        let burst = Self::capability_burst(capability, tier, window, max);
        match window {
            LimitWindow::Hourly => {
                Quota::per_hour(NonZeroU32::new(max).expect("hourly quota must be > 0"))
                    .allow_burst(NonZeroU32::new(burst).expect("hourly burst must be > 0"))
            }
            LimitWindow::Daily => {
                Quota::with_period(std::time::Duration::from_secs(86400 / max as u64))
                    .expect("daily quota must be > 0")
                    .allow_burst(NonZeroU32::new(burst).expect("daily burst must be > 0"))
            }
        }
    }

    pub fn new() -> Self {
        let capabilities = [
            RateLimitCapability::PlainArticleGeneration,
            RateLimitCapability::ResearchGeneration,
            RateLimitCapability::EditAgentRequest,
            RateLimitCapability::BackgroundTranslation,
            RateLimitCapability::ImageRegeneration,
            RateLimitCapability::ClarifyingQuestion,
        ];
        let tiers = [
            RequesterTier::Anonymous,
            RequesterTier::Authenticated,
            RequesterTier::Admin,
        ];
        let windows = [LimitWindow::Hourly, LimitWindow::Daily];

        let mut limiters = HashMap::new();
        for capability in capabilities {
            for tier in tiers {
                for window in windows {
                    limiters.insert(
                        Self::limiter_key(capability, tier, window),
                        Arc::new(KeyedLimiter::dashmap(Self::quota_for(
                            capability, tier, window,
                        ))),
                    );
                }
            }
        }

        let hit_counters = capabilities
            .into_iter()
            .map(|capability| (capability, Arc::new(AtomicU64::new(0))))
            .collect();

        Self {
            limiters: Arc::new(limiters),
            hit_counters: Arc::new(hit_counters),
            total_requests: Arc::new(AtomicU64::new(0)),
        }
    }

    fn check_capability_limit(
        &self,
        capability: RateLimitCapability,
        tier: RequesterTier,
        key: &str,
    ) -> Result<(), LimitWindow> {
        let key = key.to_string();
        let hourly_limiter = self
            .limiters
            .get(&Self::limiter_key(capability, tier, LimitWindow::Hourly))
            .expect("missing hourly limiter");
        if hourly_limiter.check_key(&key).is_err() {
            self.hit_counters
                .get(&capability)
                .expect("missing hit counter")
                .fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                capability = capability.label(),
                tier = ?tier,
                "Hourly rate limit exceeded"
            );
            return Err(LimitWindow::Hourly);
        }

        let daily_limiter = self
            .limiters
            .get(&Self::limiter_key(capability, tier, LimitWindow::Daily))
            .expect("missing daily limiter");
        if daily_limiter.check_key(&key).is_err() {
            self.hit_counters
                .get(&capability)
                .expect("missing hit counter")
                .fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                capability = capability.label(),
                tier = ?tier,
                "Daily rate limit exceeded"
            );
            return Err(LimitWindow::Daily);
        }

        Ok(())
    }

    pub fn check_article_generation_limit(
        &self,
        tier: RequesterTier,
        key: &str,
    ) -> Result<(), ArticleRateLimit> {
        self.check_capability_limit(RateLimitCapability::PlainArticleGeneration, tier, key)
            .map_err(|window| match window {
                LimitWindow::Hourly => ArticleRateLimit::Hourly,
                LimitWindow::Daily => ArticleRateLimit::Daily,
            })
    }

    pub fn check_translation_generation_limit(
        &self,
        tier: RequesterTier,
        key: &str,
    ) -> Result<(), TranslationRateLimit> {
        self.check_capability_limit(RateLimitCapability::BackgroundTranslation, tier, key)
            .map_err(|window| match window {
                LimitWindow::Hourly => TranslationRateLimit::Hourly,
                LimitWindow::Daily => TranslationRateLimit::Daily,
            })
    }
}

// Middleware function for request monitoring
pub async fn rate_limit_middleware(
    state: axum::extract::State<RateLimitState>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    state.total_requests.fetch_add(1, Ordering::Relaxed);
    Ok(next.run(request).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn article_hourly_burst_blocks_same_anonymous_key_only() {
        let state = RateLimitState::new();
        let max = RateLimitState::capability_limit(
            RateLimitCapability::PlainArticleGeneration,
            RequesterTier::Anonymous,
            LimitWindow::Hourly,
        );
        let burst = RateLimitState::capability_burst(
            RateLimitCapability::PlainArticleGeneration,
            RequesterTier::Anonymous,
            LimitWindow::Hourly,
            max,
        );
        for i in 0..burst {
            assert!(
                state
                    .check_article_generation_limit(RequesterTier::Anonymous, "anon-a")
                    .is_ok(),
                "failed at {}",
                i
            );
        }
        assert_eq!(
            state.check_article_generation_limit(RequesterTier::Anonymous, "anon-a"),
            Err(ArticleRateLimit::Hourly)
        );
        assert!(state
            .check_article_generation_limit(RequesterTier::Anonymous, "anon-b")
            .is_ok());
    }

    #[tokio::test]
    async fn authenticated_users_have_separate_article_buckets() {
        let state = RateLimitState::new();
        let max = RateLimitState::capability_limit(
            RateLimitCapability::PlainArticleGeneration,
            RequesterTier::Authenticated,
            LimitWindow::Hourly,
        );
        let burst = RateLimitState::capability_burst(
            RateLimitCapability::PlainArticleGeneration,
            RequesterTier::Authenticated,
            LimitWindow::Hourly,
            max,
        );
        for _ in 0..burst {
            assert!(state
                .check_article_generation_limit(
                    RequesterTier::Authenticated,
                    "user:author@example.com"
                )
                .is_ok());
        }

        assert_eq!(
            state.check_article_generation_limit(
                RequesterTier::Authenticated,
                "user:author@example.com"
            ),
            Err(ArticleRateLimit::Hourly)
        );
        assert!(state
            .check_article_generation_limit(RequesterTier::Authenticated, "user:other@example.com")
            .is_ok());
    }

    #[tokio::test]
    async fn translation_hourly_burst_blocks_same_key_only() {
        let state = RateLimitState::new();
        let max = RateLimitState::capability_limit(
            RateLimitCapability::BackgroundTranslation,
            RequesterTier::Anonymous,
            LimitWindow::Hourly,
        );
        let burst = RateLimitState::capability_burst(
            RateLimitCapability::BackgroundTranslation,
            RequesterTier::Anonymous,
            LimitWindow::Hourly,
            max,
        );
        for i in 0..burst {
            assert!(
                state
                    .check_translation_generation_limit(RequesterTier::Anonymous, "anon-a")
                    .is_ok(),
                "failed at {}",
                i
            );
        }
        assert_eq!(
            state.check_translation_generation_limit(RequesterTier::Anonymous, "anon-a"),
            Err(TranslationRateLimit::Hourly)
        );
        assert!(state
            .check_translation_generation_limit(RequesterTier::Anonymous, "anon-b")
            .is_ok());
    }
}
