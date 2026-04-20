use crate::llm::prompt_registry::SupportedTranslationLanguage;
use crate::rate_limit::RequesterTier;
use crate::services::article_language::PreferredLanguageSource;

pub const TRANSLATION_JOB_STATUS_QUEUED: &str = "queued";
pub const TRANSLATION_JOB_STATUS_PROCESSING: &str = "processing";
pub const TRANSLATION_JOB_STATUS_COMPLETED: &str = "completed";
pub const TRANSLATION_JOB_STATUS_FAILED: &str = "failed";
pub const TRANSLATION_JOB_STATUS_CANCELLED: &str = "cancelled";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TranslationJobRequestSource {
    Explicit,
    Cookie,
    Browser,
    EditRefresh,
}

impl TranslationJobRequestSource {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Cookie => "cookie",
            Self::Browser => "browser",
            Self::EditRefresh => "edit_refresh",
        }
    }

    pub(super) fn priority(self) -> i32 {
        match self {
            Self::Explicit => 30,
            Self::EditRefresh => 25,
            Self::Cookie => 20,
            Self::Browser => 10,
        }
    }
}

pub fn request_source_from_preferred_language(
    source: PreferredLanguageSource,
) -> TranslationJobRequestSource {
    match source {
        PreferredLanguageSource::Explicit => TranslationJobRequestSource::Explicit,
        PreferredLanguageSource::Cookie => TranslationJobRequestSource::Cookie,
        PreferredLanguageSource::Browser | PreferredLanguageSource::ArticleSource => {
            TranslationJobRequestSource::Browser
        }
    }
}

pub(super) fn request_priority(
    source: TranslationJobRequestSource,
    requester_tier: RequesterTier,
) -> i32 {
    source.priority() + requester_tier.queue_priority_boost()
}

pub(super) fn article_translation_job_id(
    article_id: &str,
    language: SupportedTranslationLanguage,
) -> String {
    crate::services::article_translations::article_translation_job_key(article_id, language)
}
