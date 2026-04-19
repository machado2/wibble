use serde::Deserialize;

use crate::error::Error;
use crate::services::editorial_policy::enforce_generation_request_policy;

use super::MAX_PROMPT_CHARS;

pub fn normalize_create_prompt(raw: &str) -> Result<String, Error> {
    let prompt = raw.trim();
    if prompt.is_empty() {
        return Err(Error::BadRequest(
            "Add a prompt before generating an article.".to_string(),
        ));
    }
    if prompt.chars().count() > MAX_PROMPT_CHARS {
        return Err(Error::BadRequest(format!(
            "Prompt is too long. Keep it under {} characters.",
            MAX_PROMPT_CHARS
        )));
    }
    enforce_generation_request_policy(prompt)?;
    Ok(prompt.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateModeSelection {
    Auto,
    Standard,
    Research,
}

impl CreateModeSelection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Standard => "standard",
            Self::Research => "research",
        }
    }

    pub fn manual_research_requested(self) -> bool {
        matches!(self, Self::Research)
    }
}

pub fn normalize_create_mode(raw: Option<&str>) -> Result<CreateModeSelection, Error> {
    match raw.unwrap_or("auto").trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(CreateModeSelection::Auto),
        "standard" => Ok(CreateModeSelection::Standard),
        "research" => Ok(CreateModeSelection::Research),
        other => Err(Error::BadRequest(format!("Unknown create mode: {}", other))),
    }
}

#[derive(Deserialize, Debug)]
pub struct PostCreateData {
    pub prompt: String,
    pub mode: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{normalize_create_mode, normalize_create_prompt, CreateModeSelection};

    #[test]
    fn create_prompt_validation_trims_and_rejects_empty_input() {
        assert_eq!(
            normalize_create_prompt("  hello wobble  ").unwrap(),
            "hello wobble"
        );
        assert!(normalize_create_prompt("   ").is_err());
    }

    #[test]
    fn create_prompt_validation_rejects_overly_long_input() {
        let prompt = "a".repeat(601);
        assert!(normalize_create_prompt(&prompt).is_err());
    }

    #[test]
    fn create_mode_defaults_to_auto() {
        assert_eq!(
            normalize_create_mode(None).unwrap(),
            CreateModeSelection::Auto
        );
    }

    #[test]
    fn create_mode_rejects_unknown_values() {
        assert!(normalize_create_mode(Some("expedition")).is_err());
    }
}
