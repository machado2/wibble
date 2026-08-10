use serde_json::json;
use serde_json::Value;
use tracing::{event, Level};

use crate::app_state::AppState;
use crate::error::Error;
use crate::services::article_jobs::{
    ArticleJobService, ARTICLE_JOB_PHASE_PLANNING, ARTICLE_JOB_PHASE_QUEUED,
    ARTICLE_JOB_PHASE_READY_FOR_REVIEW, ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_WRITING,
};

const DEFAULT_MAX_PROMPT_CHARS: usize = 600;
const DEFAULT_MAX_AGENT_STEPS: u32 = 6;
const DEFAULT_MAX_MODEL_CALLS: u32 = 3;
const DEFAULT_MAX_TOOL_CALLS: u32 = 4;
const DEFAULT_MAX_FETCHED_CONTENT_CHARS: usize = 0;
const DEFAULT_MAX_SEARCHES: u32 = 0;
const DEFAULT_MAX_SOURCES: u32 = 0;

pub const GENERATION_TOOL_REGISTRY: [GenerationTool; 5] = [
    GenerationTool::ArticlePlanning,
    GenerationTool::LimitedWebResearch,
    GenerationTool::DraftWriter,
    GenerationTool::ImageBriefPlanner,
    GenerationTool::PolicyCheck,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenerationTool {
    ArticlePlanning,
    LimitedWebResearch,
    DraftWriter,
    ImageBriefPlanner,
    PolicyCheck,
}

impl GenerationTool {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ArticlePlanning => "article_planning",
            Self::LimitedWebResearch => "limited_web_research",
            Self::DraftWriter => "draft_writer",
            Self::ImageBriefPlanner => "image_brief_planner",
            Self::PolicyCheck => "policy_check",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Self::ArticlePlanning => "Outline the article angle and structural plan",
            Self::LimitedWebResearch => {
                "Run the bounded public-web search and fetch flow for grounded briefs"
            }
            Self::DraftWriter => "Produce the article draft from the selected prompt layer",
            Self::ImageBriefPlanner => "Generate illustration briefs for the article body",
            Self::PolicyCheck => "Validate the output against formatting and safety constraints",
        }
    }

    fn phase(self) -> &'static str {
        match self {
            Self::ArticlePlanning => ARTICLE_JOB_PHASE_PLANNING,
            Self::LimitedWebResearch => ARTICLE_JOB_PHASE_RESEARCHING,
            Self::PolicyCheck => ARTICLE_JOB_PHASE_READY_FOR_REVIEW,
            Self::DraftWriter | Self::ImageBriefPlanner => ARTICLE_JOB_PHASE_WRITING,
        }
    }

    pub fn registry() -> &'static [GenerationTool] {
        &GENERATION_TOOL_REGISTRY
    }
}

#[derive(Clone, Copy, Debug)]
pub struct GenerationExecutionPolicy {
    max_prompt_chars: usize,
    max_agent_steps: u32,
    max_model_calls: u32,
    max_tool_calls: u32,
    max_fetched_content_chars: usize,
    max_searches: u32,
    max_sources: u32,
}

impl GenerationExecutionPolicy {
    pub fn standard_non_research() -> Self {
        Self {
            max_prompt_chars: DEFAULT_MAX_PROMPT_CHARS,
            max_agent_steps: DEFAULT_MAX_AGENT_STEPS,
            max_model_calls: DEFAULT_MAX_MODEL_CALLS,
            max_tool_calls: DEFAULT_MAX_TOOL_CALLS,
            max_fetched_content_chars: DEFAULT_MAX_FETCHED_CONTENT_CHARS,
            max_searches: DEFAULT_MAX_SEARCHES,
            max_sources: DEFAULT_MAX_SOURCES,
        }
    }

    pub fn research_mode() -> Self {
        Self {
            max_prompt_chars: DEFAULT_MAX_PROMPT_CHARS,
            max_agent_steps: DEFAULT_MAX_AGENT_STEPS + 2,
            max_model_calls: DEFAULT_MAX_MODEL_CALLS,
            max_tool_calls: DEFAULT_MAX_TOOL_CALLS + 1,
            max_fetched_content_chars: 12_000,
            max_searches: 1,
            max_sources: 3,
        }
    }
}

pub struct BoundedGenerationRuntime {
    job_id: String,
    job_service: ArticleJobService,
    policy: GenerationExecutionPolicy,
    prompt_chars: usize,
    agent_steps: u32,
    model_calls: u32,
    tool_calls: u32,
    searches: u32,
    sources: u32,
    fetched_content_chars: usize,
    current_tool: &'static str,
}

impl BoundedGenerationRuntime {
    pub async fn new(state: AppState, job_id: String, prompt: &str) -> Result<Self, Error> {
        Self::new_with_policy(
            state,
            job_id,
            prompt,
            GenerationExecutionPolicy::standard_non_research(),
        )
        .await
    }

    pub async fn new_research(
        state: AppState,
        job_id: String,
        prompt: &str,
    ) -> Result<Self, Error> {
        Self::new_with_policy(
            state,
            job_id,
            prompt,
            GenerationExecutionPolicy::research_mode(),
        )
        .await
    }

    async fn new_with_policy(
        state: AppState,
        job_id: String,
        prompt: &str,
        policy: GenerationExecutionPolicy,
    ) -> Result<Self, Error> {
        let prompt_chars = prompt.chars().count();
        if prompt_chars > policy.max_prompt_chars {
            return Err(Error::BadRequest(format!(
                "Prompt exceeds runtime limit of {} characters",
                policy.max_prompt_chars
            )));
        }

        let runtime = Self {
            job_id,
            job_service: ArticleJobService::new(state),
            policy,
            prompt_chars,
            agent_steps: 0,
            model_calls: 0,
            tool_calls: 0,
            searches: 0,
            sources: 0,
            fetched_content_chars: 0,
            current_tool: "queued",
        };
        runtime.persist(ARTICLE_JOB_PHASE_QUEUED).await?;
        Ok(runtime)
    }

    pub async fn begin_tool(
        &mut self,
        tool: GenerationTool,
        uses_model: bool,
    ) -> Result<(), Error> {
        self.ensure_not_cancelled().await?;
        self.agent_steps += 1;
        if self.agent_steps > self.policy.max_agent_steps {
            return Err(Error::Llm(format!(
                "Article generation exceeded max agent steps ({})",
                self.policy.max_agent_steps
            )));
        }

        self.tool_calls += 1;
        if self.tool_calls > self.policy.max_tool_calls {
            return Err(Error::Llm(format!(
                "Article generation exceeded max tool calls ({})",
                self.policy.max_tool_calls
            )));
        }

        if uses_model {
            self.model_calls += 1;
            if self.model_calls > self.policy.max_model_calls {
                return Err(Error::Llm(format!(
                    "Article generation exceeded max model calls ({})",
                    self.policy.max_model_calls
                )));
            }
        }

        self.current_tool = tool.as_str();
        event!(
            Level::INFO,
            job_id = %self.job_id,
            tool = tool.as_str(),
            agent_steps = self.agent_steps,
            model_calls = self.model_calls,
            tool_calls = self.tool_calls,
            "Article generation tool invoked"
        );
        self.persist(tool.phase()).await
    }

    pub async fn record_fetched_content(&mut self, chars: usize) -> Result<(), Error> {
        self.ensure_not_cancelled().await?;
        self.fetched_content_chars = self.fetched_content_chars.saturating_add(chars);
        if self.fetched_content_chars > self.policy.max_fetched_content_chars {
            return Err(Error::Llm(format!(
                "Article generation exceeded fetched content budget ({})",
                self.policy.max_fetched_content_chars
            )));
        }
        self.persist(ARTICLE_JOB_PHASE_PLANNING).await
    }

    pub async fn record_search(&mut self) -> Result<(), Error> {
        self.ensure_not_cancelled().await?;
        self.searches = self.searches.saturating_add(1);
        if self.searches > self.policy.max_searches {
            return Err(Error::Llm(format!(
                "Article generation exceeded search budget ({})",
                self.policy.max_searches
            )));
        }
        self.persist(ARTICLE_JOB_PHASE_PLANNING).await
    }

    pub async fn record_source(&mut self) -> Result<(), Error> {
        self.ensure_not_cancelled().await?;
        self.sources = self.sources.saturating_add(1);
        if self.sources > self.policy.max_sources {
            return Err(Error::Llm(format!(
                "Article generation exceeded source budget ({})",
                self.policy.max_sources
            )));
        }
        self.persist(ARTICLE_JOB_PHASE_PLANNING).await
    }

    pub async fn mark_ready_for_review(&mut self) -> Result<(), Error> {
        self.ensure_not_cancelled().await?;
        self.current_tool = GenerationTool::PolicyCheck.as_str();
        self.persist(ARTICLE_JOB_PHASE_READY_FOR_REVIEW).await
    }

    pub async fn ensure_not_cancelled(&self) -> Result<(), Error> {
        if self.job_service.is_job_cancelled(&self.job_id).await? {
            return Err(Error::BadRequest(format!(
                "Article job {} was cancelled",
                self.job_id
            )));
        }
        Ok(())
    }

    async fn persist(&self, phase: &str) -> Result<(), Error> {
        self.job_service
            .record_usage_snapshot(&self.job_id, phase, self.snapshot())
            .await
    }

    fn snapshot(&self) -> Value {
        json!({
            "prompt_chars": self.prompt_chars,
            "agent_steps": self.agent_steps,
            "model_calls": self.model_calls,
            "tool_calls": self.tool_calls,
            "searches": self.searches,
            "sources": self.sources,
            "fetched_content_chars": self.fetched_content_chars,
            "current_tool": self.current_tool,
            "limits": {
                "max_prompt_chars": self.policy.max_prompt_chars,
                "max_agent_steps": self.policy.max_agent_steps,
                "max_model_calls": self.policy.max_model_calls,
                "max_tool_calls": self.policy.max_tool_calls,
                "max_fetched_content_chars": self.policy.max_fetched_content_chars,
                "max_searches": self.policy.max_searches,
                "max_sources": self.policy.max_sources,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GenerationExecutionPolicy, GenerationTool};
    use crate::services::article_jobs::{
        ARTICLE_JOB_PHASE_PLANNING, ARTICLE_JOB_PHASE_READY_FOR_REVIEW,
        ARTICLE_JOB_PHASE_RESEARCHING, ARTICLE_JOB_PHASE_WRITING,
    };

    #[test]
    fn standard_non_research_policy_disables_fetch_and_search_by_default() {
        let policy = GenerationExecutionPolicy::standard_non_research();

        assert_eq!(policy.max_fetched_content_chars, 0);
        assert_eq!(policy.max_searches, 0);
        assert_eq!(policy.max_sources, 0);
    }

    #[test]
    fn generation_tools_map_to_structured_job_phases() {
        assert_eq!(
            GenerationTool::ArticlePlanning.phase(),
            ARTICLE_JOB_PHASE_PLANNING
        );
        assert_eq!(
            GenerationTool::LimitedWebResearch.phase(),
            ARTICLE_JOB_PHASE_RESEARCHING
        );
        assert_eq!(
            GenerationTool::DraftWriter.phase(),
            ARTICLE_JOB_PHASE_WRITING
        );
        assert_eq!(
            GenerationTool::ImageBriefPlanner.phase(),
            ARTICLE_JOB_PHASE_WRITING
        );
        assert_eq!(
            GenerationTool::PolicyCheck.phase(),
            ARTICLE_JOB_PHASE_READY_FOR_REVIEW
        );
    }

    #[test]
    fn research_policy_enables_bounded_search_and_source_budgets() {
        let policy = GenerationExecutionPolicy::research_mode();

        assert_eq!(policy.max_searches, 1);
        assert_eq!(policy.max_sources, 3);
        assert!(policy.max_fetched_content_chars > 0);
    }

    #[test]
    fn generation_tool_registry_covers_structured_runtime_steps() {
        let names = GenerationTool::registry()
            .iter()
            .map(|tool| tool.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "article_planning",
                "limited_web_research",
                "draft_writer",
                "image_brief_planner",
                "policy_check",
            ]
        );
        assert!(GenerationTool::registry()
            .iter()
            .all(|tool| !tool.description().is_empty()));
    }
}
