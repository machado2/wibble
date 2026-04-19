use crate::error::Error;

const PRIVATE_INDIVIDUAL_MARKERS: &[&str] = &[
    "my neighbor",
    "my ex",
    "my boss",
    "my teacher",
    "my coworker",
    "my colleague",
    "my classmate",
    "my friend",
    "private citizen",
    "local resident",
];

const HIGH_RISK_EVENT_MARKERS: &[&str] = &[
    "mass shooting",
    "school shooting",
    "terror attack",
    "terrorist attack",
    "suicide bombing",
    "hostage crisis",
    "ethnic cleansing",
    "genocide",
];

const ALLEGATION_MARKERS: &[&str] = &[
    "accused of",
    "allegedly",
    "allegation",
    "allegations",
    "sexual assault",
    "rape accusation",
    "murder accusation",
    "embezzlement",
    "embezzled",
    "fraud accusation",
    "pedophile",
];

const PERSON_REFERENCE_MARKERS: &[&str] = &[
    "mr ",
    "mrs ",
    "ms ",
    "dr ",
    "senator ",
    "minister ",
    "president ",
    "prime minister ",
    "mayor ",
    "governor ",
];

pub fn enforce_generation_request_policy(prompt: &str) -> Result<(), Error> {
    enforce_request_policy(prompt, "prompt")
}

pub fn enforce_edit_request_policy(change_request: &str) -> Result<(), Error> {
    enforce_request_policy(change_request, "edit request")
}

pub fn enforce_article_output_policy(
    title: &str,
    description: &str,
    markdown: &str,
) -> Result<(), Error> {
    let combined = format!("{}\n{}\n{}", title, description, markdown);
    if contains_high_risk_event(&combined) {
        return Err(Error::Llm(
            "Generated article touches a blocked high-risk contemporary event".to_string(),
        ));
    }
    if contains_private_individual_target(&combined) {
        return Err(Error::Llm(
            "Generated article targets a private individual, which is not allowed".to_string(),
        ));
    }
    if contains_real_person_allegation(&combined) {
        return Err(Error::Llm(
            "Generated article frames a real-person allegation, which is not allowed".to_string(),
        ));
    }
    Ok(())
}

fn enforce_request_policy(input: &str, context: &str) -> Result<(), Error> {
    if contains_private_individual_target(input) {
        return Err(Error::BadRequest(format!(
            "This {} appears to target a private individual, which is not allowed.",
            context
        )));
    }
    if contains_real_person_allegation(input) {
        return Err(Error::BadRequest(format!(
            "This {} frames a real-person allegation, which is not allowed.",
            context
        )));
    }
    if contains_high_risk_event(input) {
        return Err(Error::BadRequest(format!(
            "This {} references a blocked high-risk contemporary event.",
            context
        )));
    }
    Ok(())
}

fn contains_private_individual_target(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    PRIVATE_INDIVIDUAL_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn contains_high_risk_event(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    HIGH_RISK_EVENT_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
}

fn contains_real_person_allegation(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    let contains_allegation = ALLEGATION_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker));
    if !contains_allegation {
        return false;
    }

    PERSON_REFERENCE_MARKERS
        .iter()
        .any(|marker| normalized.contains(marker))
        || looks_like_named_person(text)
        || contains_private_individual_target(text)
}

fn looks_like_named_person(text: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::{
        enforce_article_output_policy, enforce_edit_request_policy,
        enforce_generation_request_policy,
    };

    #[test]
    fn rejects_private_individual_generation_targets() {
        let err = enforce_generation_request_policy(
            "Write about my neighbor being appointed minister of parking",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("private individual"));
    }

    #[test]
    fn rejects_real_person_allegations_in_edit_requests() {
        let err = enforce_edit_request_policy(
            "Make it about Mayor John Smith being accused of embezzlement",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("real-person allegation"));
    }

    #[test]
    fn rejects_high_risk_output() {
        let err = enforce_article_output_policy(
            "Cabinet update",
            "Description",
            "Paragraph about a mass shooting response",
        )
        .unwrap_err()
        .to_string();

        assert!(err.contains("high-risk"));
    }
}
