use std::env;

use chrono::{Duration, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::error::Error;

const DEFAULT_CLARIFICATION_TIMEOUT_SECONDS: i64 = 5 * 60;
const MAX_CLARIFICATION_ANSWER_CHARS: usize = 200;

const INSTITUTION_MARKERS: &[&str] = &[
    "ministry",
    "department",
    "council",
    "committee",
    "agency",
    "office",
    "authority",
    "board",
    "cabinet",
    "parliament",
    "court",
    "police",
    "hospital",
    "school",
    "university",
    "federation",
    "commission",
    "tribunal",
    "institute",
    "company",
    "union",
];

const RESPONSE_MARKERS: &[&str] = &[
    "announces",
    "announced",
    "orders",
    "ordered",
    "launches",
    "launched",
    "opens",
    "opened",
    "issues",
    "issued",
    "publishes",
    "published",
    "review",
    "inquiry",
    "guidance",
    "policy",
    "memo",
    "report",
];

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ClarificationRequest {
    pub question: String,
    pub fallback_instruction: String,
    pub auto_resume_at: String,
    pub clarification_count: u32,
}

impl ClarificationRequest {
    pub fn auto_resume_at_datetime(&self) -> Option<NaiveDateTime> {
        NaiveDateTime::parse_from_str(&self.auto_resume_at, "%F %T").ok()
    }

    pub fn formatted_deadline(&self) -> Option<String> {
        self.auto_resume_at_datetime()
            .map(|value| value.format("%F %T").to_string())
    }
}

pub fn build_clarification_request(prompt: &str) -> Option<ClarificationRequest> {
    if !should_request_clarification(prompt) {
        return None;
    }

    let normalized = prompt.to_ascii_lowercase();
    let question = if !contains_any(&normalized, INSTITUTION_MARKERS) {
        "Which institution, office, or organization should treat this as routine?"
    } else {
        "What specific policy decision or official response should the article center?"
    };
    let fallback_instruction = "If no answer arrives, choose the most plausible public-institution framing and continue with one clear official response.".to_string();
    let auto_resume_at = (chrono::Utc::now().naive_local()
        + Duration::seconds(clarification_timeout_seconds()))
    .format("%F %T")
    .to_string();

    Some(ClarificationRequest {
        question: question.to_string(),
        fallback_instruction,
        auto_resume_at,
        clarification_count: 1,
    })
}

pub fn parse_clarification_request(value: Option<&str>) -> Option<ClarificationRequest> {
    value.and_then(|value| serde_json::from_str(value).ok())
}

pub fn normalize_clarification_answer(raw: &str) -> Result<String, Error> {
    let answer = raw.trim();
    if answer.is_empty() {
        return Err(Error::BadRequest(
            "Answer the clarification question before resuming generation.".to_string(),
        ));
    }
    if answer.chars().count() > MAX_CLARIFICATION_ANSWER_CHARS {
        return Err(Error::BadRequest(format!(
            "Clarification answers must stay under {} characters.",
            MAX_CLARIFICATION_ANSWER_CHARS
        )));
    }
    Ok(answer.to_string())
}

pub fn append_clarification_answer(prompt: &str, question: &str, answer: &str) -> String {
    format!("{prompt}\n\nClarification from requester:\nQuestion: {question}\nAnswer: {answer}")
}

pub fn append_clarification_fallback(prompt: &str, fallback_instruction: &str) -> String {
    format!("{prompt}\n\nClarification timeout fallback:\n{fallback_instruction}")
}

pub fn clarification_timeout_seconds() -> i64 {
    env::var("ARTICLE_CLARIFICATION_TIMEOUT_SECONDS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_CLARIFICATION_TIMEOUT_SECONDS)
}

fn should_request_clarification(prompt: &str) -> bool {
    let normalized = prompt.to_ascii_lowercase();
    let word_count = prompt.split_whitespace().count();
    let has_institution = contains_any(&normalized, INSTITUTION_MARKERS);
    let has_response = contains_any(&normalized, RESPONSE_MARKERS);

    word_count < 12 || !has_institution || !has_response
}

fn contains_any(text: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| text.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::{
        append_clarification_answer, append_clarification_fallback, build_clarification_request,
        normalize_clarification_answer,
    };

    #[test]
    fn clarification_request_triggers_for_materially_ambiguous_prompt() {
        let request = build_clarification_request("A local office begins doing something odd")
            .expect("expected clarification request");

        assert!(request.question.contains("policy"));
        assert_eq!(request.clarification_count, 1);
    }

    #[test]
    fn clarification_request_skips_specific_prompt() {
        let request = build_clarification_request(
            "A transport ministry issues a formal policy memo announcing emotional readiness bulletins for rail delays.",
        );

        assert!(request.is_none());
    }

    #[test]
    fn clarification_answer_validation_rejects_empty_values() {
        assert!(normalize_clarification_answer("   ").is_err());
    }

    #[test]
    fn clarification_answer_is_appended_to_prompt() {
        let prompt =
            append_clarification_answer("Original", "Which office?", "The transport ministry");

        assert!(prompt.contains("Clarification from requester"));
        assert!(prompt.contains("The transport ministry"));
    }

    #[test]
    fn clarification_fallback_is_appended_to_prompt() {
        let prompt =
            append_clarification_fallback("Original", "Choose the most plausible ministry");

        assert!(prompt.contains("Clarification timeout fallback"));
        assert!(prompt.contains("most plausible ministry"));
    }
}
