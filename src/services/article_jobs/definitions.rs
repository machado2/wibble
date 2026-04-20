use crate::app_state::AppState;
use crate::llm::article_generator::ResearchModeSource;
use crate::rate_limit::RequesterTier;

pub const ARTICLE_JOB_STATUS_QUEUED: &str = "queued";
pub const ARTICLE_JOB_STATUS_PROCESSING: &str = "processing";
pub const ARTICLE_JOB_STATUS_COMPLETED: &str = "completed";
pub const ARTICLE_JOB_STATUS_FAILED: &str = "failed";
pub const ARTICLE_JOB_STATUS_CANCELLED: &str = "cancelled";

pub const ARTICLE_JOB_PHASE_QUEUED: &str = "queued";
pub const ARTICLE_JOB_PHASE_PLANNING: &str = "planning";
pub const ARTICLE_JOB_PHASE_RESEARCHING: &str = "researching";
pub const ARTICLE_JOB_PHASE_AWAITING_USER_INPUT: &str = "awaiting_user_input";
pub const ARTICLE_JOB_PHASE_WRITING: &str = "writing";
pub const ARTICLE_JOB_PHASE_EDITING: &str = "editing";
pub const ARTICLE_JOB_PHASE_TRANSLATING: &str = "translating";
pub const ARTICLE_JOB_PHASE_RENDERING_IMAGES: &str = "rendering_images";
pub const ARTICLE_JOB_PHASE_READY_FOR_REVIEW: &str = "ready_for_review";
pub const ARTICLE_JOB_PHASE_COMPLETED: &str = "completed";
pub const ARTICLE_JOB_PHASE_FAILED: &str = "failed";
pub const ARTICLE_JOB_PHASE_CANCELLED: &str = "cancelled";

#[derive(Clone)]
pub struct ArticleJobService {
    pub(super) state: AppState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArticleJobFeatureType {
    Create,
    CreateResearchAuto,
    CreateResearchManual,
    DeadLinkRecovery,
}

impl ArticleJobFeatureType {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::CreateResearchAuto => "create_research_auto",
            Self::CreateResearchManual => "create_research_manual",
            Self::DeadLinkRecovery => "dead_link_recovery",
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            "create" => Some(Self::Create),
            "create_research_auto" => Some(Self::CreateResearchAuto),
            "create_research_manual" => Some(Self::CreateResearchManual),
            "dead_link_recovery" => Some(Self::DeadLinkRecovery),
            _ => None,
        }
    }

    pub(super) fn from_research_mode(mode: Option<ResearchModeSource>) -> Self {
        match mode {
            Some(ResearchModeSource::Auto) => Self::CreateResearchAuto,
            Some(ResearchModeSource::Manual) => Self::CreateResearchManual,
            None => Self::Create,
        }
    }

    pub(super) fn research_mode(self) -> Option<ResearchModeSource> {
        match self {
            Self::Create => None,
            Self::CreateResearchAuto => Some(ResearchModeSource::Auto),
            Self::CreateResearchManual => Some(ResearchModeSource::Manual),
            Self::DeadLinkRecovery => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArticleJobRequest {
    pub(super) article_id: Option<String>,
    pub(super) requester_key: String,
    pub(super) requester_tier: String,
    pub(super) author_email: Option<String>,
    pub(super) prompt: String,
    pub(super) feature_type: ArticleJobFeatureType,
}

impl ArticleJobRequest {
    pub fn create(
        prompt: String,
        author_email: Option<String>,
        requester_tier: RequesterTier,
        rate_limit_key: String,
        research_mode: Option<ResearchModeSource>,
    ) -> Self {
        Self {
            article_id: None,
            requester_key: rate_limit_key,
            requester_tier: requester_tier_label(requester_tier).to_string(),
            author_email,
            prompt,
            feature_type: ArticleJobFeatureType::from_research_mode(research_mode),
        }
    }

    pub fn dead_link_recovery(prompt: String, article_id: String) -> Self {
        Self {
            article_id: Some(article_id),
            requester_key: "system:dead_link_recovery".to_string(),
            requester_tier: "SYSTEM".to_string(),
            author_email: None,
            prompt,
            feature_type: ArticleJobFeatureType::DeadLinkRecovery,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ArticleJobTrace {
    pub(super) job_kind: &'static str,
    pub(super) recovery_slug: Option<String>,
}

impl ArticleJobTrace {
    pub fn create(research_mode: Option<ResearchModeSource>) -> Self {
        let feature_type = ArticleJobFeatureType::from_research_mode(research_mode);
        Self {
            job_kind: feature_type.as_str(),
            recovery_slug: None,
        }
    }

    pub fn dead_link_recovery(slug: String) -> Self {
        Self {
            job_kind: ArticleJobFeatureType::DeadLinkRecovery.as_str(),
            recovery_slug: Some(slug),
        }
    }

    pub(super) fn from_job(job: &crate::entities::article_job::Model) -> Self {
        match ArticleJobFeatureType::from_str(&job.feature_type) {
            Some(ArticleJobFeatureType::DeadLinkRecovery) => Self {
                job_kind: ArticleJobFeatureType::DeadLinkRecovery.as_str(),
                recovery_slug: None,
            },
            Some(feature_type) => Self {
                job_kind: feature_type.as_str(),
                recovery_slug: None,
            },
            None => Self::create(None),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ImageProgress {
    pub(super) total: usize,
    pub(super) completed: usize,
    pub(super) processing: usize,
    pub(super) failed: usize,
    pub(super) pending_ids: Vec<String>,
}

impl ImageProgress {
    pub(super) fn has_pending(&self) -> bool {
        !self.pending_ids.is_empty()
    }
}

pub fn is_in_progress_job_status(status: &str) -> bool {
    matches!(
        status,
        ARTICLE_JOB_STATUS_QUEUED | ARTICLE_JOB_STATUS_PROCESSING
    )
}

pub fn is_terminal_job_status(status: &str) -> bool {
    matches!(
        status,
        ARTICLE_JOB_STATUS_COMPLETED | ARTICLE_JOB_STATUS_FAILED | ARTICLE_JOB_STATUS_CANCELLED
    )
}

pub(super) fn requester_tier_label(requester_tier: RequesterTier) -> &'static str {
    match requester_tier {
        RequesterTier::Anonymous => "ANON",
        RequesterTier::Authenticated => "AUTH",
        RequesterTier::Admin => "ADMIN",
    }
}
