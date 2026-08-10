use regex::Regex;
use sea_orm::{ActiveValue, EntityTrait};
use serde::Serialize;
use serde_json::Value;
use url::Url;
use uuid::Uuid;

use crate::app_state::AppState;
use crate::entities::{prelude::*, search_history};
use crate::error::Error;
use crate::llm::prompt_registry::research_article_generation_prompt;
use crate::rate_limit::RequesterTier;

use super::runtime::{BoundedGenerationRuntime, GenerationTool};

const MAX_RESEARCH_RESULTS: usize = 5;
const MAX_RESEARCH_SOURCES: usize = 3;
const MAX_SOURCE_CONTEXT_CHARS: usize = 900;
const PUBLIC_TOPIC_MARKERS: &[&str] = &[
    "minister",
    "ministry",
    "department",
    "government",
    "parliament",
    "cabinet",
    "agency",
    "authority",
    "council",
    "committee",
    "policy",
    "regulation",
    "bill",
    "law",
    "union",
    "company",
    "corporation",
    "university",
    "hospital",
    "election",
    "president",
    "prime minister",
    "mayor",
    "governor",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub enum ResearchModeSource {
    Auto,
    Manual,
}

impl ResearchModeSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ResearchSource {
    pub title: String,
    pub url: String,
    pub domain: String,
    pub snippet: String,
    pub context: String,
}

#[derive(Clone, Debug)]
pub struct ResearchPacket {
    pub mode_source: ResearchModeSource,
    pub sources: Vec<ResearchSource>,
}

impl ResearchPacket {
    pub fn prompt_context(&self, prompt: &str) -> String {
        let mut sections = vec![format!("Original brief:\n{}", prompt)];
        let source_lines = self
            .sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                format!(
                    "{}. {} ({})\nContext: {}",
                    index + 1,
                    source.title,
                    source.domain,
                    source.context
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        sections.push(format!(
            "Research file:\n{}\n\nUse only this file to ground real institutions, timelines, and named public actors. Do not cite or mention the file directly.",
            source_lines
        ));
        sections.join("\n\n")
    }

    pub fn preview_payload(&self) -> Value {
        serde_json::json!({
            "research": {
                "mode": self.mode_source.as_str(),
                "source_count": self.sources.len(),
                "prompt_version": research_article_generation_prompt().version,
                "sources": self.sources.iter().map(|source| {
                    serde_json::json!({
                        "title": source.title,
                        "url": source.url,
                        "domain": source.domain,
                    })
                }).collect::<Vec<_>>(),
            }
        })
    }
}

#[derive(Clone, Debug)]
struct SearchResult {
    title: String,
    url: String,
    domain: String,
    snippet: String,
    score: i32,
}

pub fn prompt_requires_research(prompt: &str) -> bool {
    let normalized = prompt.to_ascii_lowercase();
    let word_count = prompt.split_whitespace().count();
    word_count >= 12
        && (contains_any(&normalized, PUBLIC_TOPIC_MARKERS)
            || looks_like_named_public_entity(prompt))
}

pub fn resolve_research_mode(
    prompt: &str,
    manual_request: bool,
    requester_tier: RequesterTier,
) -> Result<Option<ResearchModeSource>, Error> {
    if manual_request {
        if requester_tier == RequesterTier::Anonymous {
            return Err(Error::Auth(
                "Log in to use the bounded research desk.".to_string(),
            ));
        }
        return Ok(Some(ResearchModeSource::Manual));
    }
    if requester_tier == RequesterTier::Anonymous {
        return Ok(None);
    }
    if prompt_requires_research(prompt) {
        return Ok(Some(ResearchModeSource::Auto));
    }
    Ok(None)
}

pub async fn gather_research_packet(
    state: &AppState,
    runtime: &mut BoundedGenerationRuntime,
    prompt: &str,
    mode_source: ResearchModeSource,
) -> Result<ResearchPacket, Error> {
    runtime
        .begin_tool(GenerationTool::LimitedWebResearch, false)
        .await?;
    runtime.record_search().await?;

    let client = reqwest::Client::builder()
        .user_agent("WibbleResearch/1.0")
        .build()
        .map_err(|e| Error::Llm(format!("Failed to build research HTTP client: {}", e)))?;
    let results = search_public_web(&client, prompt).await?;
    persist_search_history(&state.db, prompt, results.len() as i32).await?;

    let mut sources = Vec::new();
    for result in results {
        if sources.len() >= MAX_RESEARCH_SOURCES {
            break;
        }
        if let Ok(context) = fetch_source_context(&client, &result.url).await {
            runtime.record_source().await?;
            runtime
                .record_fetched_content(context.chars().count())
                .await?;
            sources.push(ResearchSource {
                title: result.title,
                url: result.url,
                domain: result.domain,
                snippet: result.snippet,
                context,
            });
        }
    }

    if sources.is_empty() {
        return Err(Error::Llm(
            "Research mode could not gather any usable public-source context".to_string(),
        ));
    }

    Ok(ResearchPacket {
        mode_source,
        sources,
    })
}

async fn search_public_web(
    client: &reqwest::Client,
    query: &str,
) -> Result<Vec<SearchResult>, Error> {
    let response = client
        .get("https://duckduckgo.com/html/")
        .query(&[("q", query)])
        .send()
        .await
        .map_err(|e| Error::Llm(format!("Failed to search public web: {}", e)))?
        .text()
        .await
        .map_err(|e| Error::Llm(format!("Failed to read search response: {}", e)))?;

    let entry_re = Regex::new(
        r#"(?s)result__a" href="(?P<href>[^"]+)".*?>(?P<title>.*?)</a>.*?result__snippet[^>]*>(?P<snippet>.*?)</"#,
    )
    .expect("search result regex must compile");
    let tag_re = Regex::new(r"<[^>]+>").expect("tag stripping regex must compile");

    let mut results = entry_re
        .captures_iter(&response)
        .filter_map(|capture| {
            let href = capture.name("href")?.as_str();
            let url = resolve_duckduckgo_url(href)?;
            let domain = Url::parse(&url)
                .ok()
                .and_then(|value| value.domain().map(str::to_string))
                .unwrap_or_default();
            Some(SearchResult {
                title: decode_html_entities(
                    &tag_re.replace_all(capture.name("title")?.as_str(), ""),
                ),
                url: url.clone(),
                domain: domain.clone(),
                snippet: decode_html_entities(
                    &tag_re.replace_all(capture.name("snippet")?.as_str(), ""),
                ),
                score: source_domain_score(&domain),
            })
        })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut seen = std::collections::HashSet::new();
    results.retain(|result| seen.insert(result.url.clone()));
    results.truncate(MAX_RESEARCH_RESULTS);
    Ok(results)
}

async fn fetch_source_context(client: &reqwest::Client, url: &str) -> Result<String, Error> {
    let stripped = url
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    let mirror_url = format!("https://r.jina.ai/http://{}", stripped);
    let response = client
        .get(&mirror_url)
        .send()
        .await
        .map_err(|e| Error::Llm(format!("Failed to fetch source text: {}", e)))?
        .text()
        .await
        .map_err(|e| Error::Llm(format!("Failed to read source text: {}", e)))?;

    let body = response
        .split_once("Markdown Content:")
        .map(|(_, body)| body)
        .unwrap_or(response.as_str());
    let collapsed = collapse_whitespace(body);
    if collapsed.is_empty() {
        return Err(Error::Llm("Fetched source text was empty".to_string()));
    }
    Ok(truncate_chars(&collapsed, MAX_SOURCE_CONTEXT_CHARS))
}

async fn persist_search_history(
    db: &sea_orm::DatabaseConnection,
    term: &str,
    result_count: i32,
) -> Result<(), Error> {
    SearchHistory::insert(search_history::ActiveModel {
        id: ActiveValue::set(Uuid::new_v4().to_string()),
        term: ActiveValue::set(term.to_string()),
        created_at: ActiveValue::set(chrono::Utc::now().naive_local()),
        result_count: ActiveValue::set(result_count),
    })
    .exec(db)
    .await
    .map_err(|e| Error::Database(format!("Error inserting search history: {}", e)))?;
    Ok(())
}

fn resolve_duckduckgo_url(raw: &str) -> Option<String> {
    let href = if raw.starts_with("//") {
        format!("https:{}", raw)
    } else {
        raw.to_string()
    };
    let parsed = Url::parse(&href).ok()?;
    let uddg = parsed
        .query_pairs()
        .find(|(key, _)| key == "uddg")
        .map(|(_, value)| value.to_string())?;
    if uddg.starts_with("http://") || uddg.starts_with("https://") {
        Some(uddg)
    } else {
        None
    }
}

fn source_domain_score(domain: &str) -> i32 {
    if domain.ends_with(".gov")
        || domain.contains(".gov.")
        || domain.contains("parliament")
        || domain.contains("senate")
        || domain.contains("ministry")
        || domain.ends_with(".edu")
    {
        30
    } else if [
        "reuters.com",
        "apnews.com",
        "bbc.com",
        "npr.org",
        "ft.com",
        "economist.com",
    ]
    .iter()
    .any(|candidate| domain.contains(candidate))
    {
        20
    } else {
        10
    }
}

fn looks_like_named_public_entity(text: &str) -> bool {
    let mut previous_capitalized = false;
    for token in text.split_whitespace() {
        let cleaned = token.trim_matches(|c: char| !c.is_alphabetic());
        let mut chars = cleaned.chars();
        let Some(first) = chars.next() else {
            previous_capitalized = false;
            continue;
        };
        let is_capitalized = first.is_uppercase() && chars.all(|c| c.is_lowercase());
        if previous_capitalized && is_capitalized {
            return true;
        }
        previous_capitalized = is_capitalized;
    }
    false
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn decode_html_entities(input: &str) -> String {
    input
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

#[cfg(test)]
mod tests {
    use crate::{error::Error, rate_limit::RequesterTier};

    use super::{
        prompt_requires_research, resolve_duckduckgo_url, resolve_research_mode,
        source_domain_score, ResearchModeSource,
    };

    #[test]
    fn research_detection_triggers_for_public_policy_topics() {
        assert!(prompt_requires_research(
            "The transport ministry unveils a new emissions policy after a parliamentary review"
        ));
    }

    #[test]
    fn research_detection_skips_generic_local_absurdity() {
        assert!(!prompt_requires_research(
            "A sandwich board outside the launderette starts issuing emotional notices"
        ));
    }

    #[test]
    fn duckduckgo_redirect_url_is_decoded() {
        let decoded = resolve_duckduckgo_url(
            "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.gov%2Fmemo&rut=test",
        )
        .unwrap();

        assert_eq!(decoded, "https://example.gov/memo");
    }

    #[test]
    fn primary_domains_rank_above_generic_results() {
        assert!(source_domain_score("transport.gov") > source_domain_score("example.com"));
    }

    #[test]
    fn manual_research_requires_login() {
        let err = resolve_research_mode(
            "The transport ministry publishes a policy update after a parliamentary review",
            true,
            RequesterTier::Anonymous,
        )
        .unwrap_err();

        assert!(matches!(err, Error::Auth(_)));
    }

    #[test]
    fn authenticated_public_briefs_auto_enable_research() {
        let mode = resolve_research_mode(
            "The transport ministry publishes a detailed policy update after a parliamentary review of regional emissions targets",
            false,
            RequesterTier::Authenticated,
        )
        .unwrap();

        assert_eq!(mode, Some(ResearchModeSource::Auto));
    }
}
