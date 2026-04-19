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

#[derive(Deserialize, Debug)]
pub struct PostCreateData {
    pub prompt: String,
}

#[cfg(test)]
mod tests {
    use super::normalize_create_prompt;

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
}
