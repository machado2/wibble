use serde_json::Value;

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;
use crate::llm::prompt_registry::{
    find_supported_translation_language, supported_translation_languages, translation_prompt,
    SupportedTranslationLanguage,
};
use crate::llm::{function_definition, Llm, Message};

const ENGLISH_LANGUAGE_CODE: &str = "en";

#[allow(async_fn_in_trait)]
pub trait Translate {
    async fn translate(&self, text: &str, target_language: &str) -> Result<String, Error>;
}

pub struct TranslationService<'a> {
    llm: &'a Llm,
}

pub struct TranslationResult {
    pub text: String,
    pub target_language: SupportedTranslationLanguage,
    pub prompt_version: i32,
}

fn send_translated_text() -> FunctionDefinition {
    let mut f = function_definition::def_function("send_translated_text", "Send translated text");
    f.parameters.add_str("text", true, "Translated text");
    f
}

pub fn translation_service(llm: &Llm) -> TranslationService<'_> {
    TranslationService { llm }
}

pub fn detect_browser_translation_language(
    header_value: Option<&str>,
) -> Option<SupportedTranslationLanguage> {
    let header_value = header_value?;
    let candidates = header_value
        .split(',')
        .enumerate()
        .filter_map(|(position, entry)| parse_accept_language_entry(entry, position));
    let mut candidates = candidates.collect::<Vec<_>>();

    candidates.sort_by(|left, right| {
        right
            .quality
            .cmp(&left.quality)
            .then(left.position.cmp(&right.position))
    });

    candidates
        .into_iter()
        .find_map(|candidate| find_supported_translation_language(candidate.tag))
}

pub fn default_translation_fallback_language(
    source_language: SupportedTranslationLanguage,
) -> SupportedTranslationLanguage {
    if source_language.code == ENGLISH_LANGUAGE_CODE {
        source_language
    } else {
        find_supported_translation_language(ENGLISH_LANGUAGE_CODE)
            .expect("English must remain in the supported translation whitelist")
    }
}

impl<'a> TranslationService<'a> {
    pub fn supported_languages(&self) -> &'static [SupportedTranslationLanguage] {
        supported_translation_languages()
    }

    pub async fn translate_text(
        &self,
        text: &str,
        target_language: &str,
    ) -> Result<TranslationResult, Error> {
        let target_language =
            find_supported_translation_language(target_language).ok_or_else(|| {
                Error::BadRequest(format!(
                    "Unsupported translation target language: {}",
                    target_language
                ))
            })?;
        let prompt = translation_prompt();
        let messages = vec![
            Message::System(prompt.body.to_string()),
            Message::User(format!(
                "Translate the following text to {} ({}):\n\n{}",
                target_language.name, target_language.code, text
            )),
        ];
        let translation = self
            .llm
            .request_tool(send_translated_text(), messages, &self.llm.models[0])
            .await?;
        let value: Value = serde_json::from_str(&translation)
            .map_err(|e| Error::Llm(format!("Failed to parse translation response: {}", e)))?;
        let text = value["text"]
            .as_str()
            .ok_or(Error::Llm("Translation response missing text".into()))?
            .to_string();

        Ok(TranslationResult {
            text,
            target_language,
            prompt_version: prompt.version,
        })
    }
}

impl Translate for Llm {
    async fn translate(&self, text: &str, target_language: &str) -> Result<String, Error> {
        translation_service(self)
            .translate_text(text, target_language)
            .await
            .map(|translation| translation.text)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedAcceptLanguage<'a> {
    tag: &'a str,
    quality: u16,
    position: usize,
}

fn parse_accept_language_entry(entry: &str, position: usize) -> Option<ParsedAcceptLanguage<'_>> {
    let mut parts = entry.split(';');
    let tag = parts.next()?.trim();
    if tag.is_empty() || tag == "*" {
        return None;
    }

    let quality = parts
        .find_map(|parameter| {
            parameter
                .trim()
                .strip_prefix("q=")
                .and_then(parse_quality_value)
        })
        .unwrap_or(1000);

    if quality == 0 {
        return None;
    }

    let supported_tag = tag
        .split('-')
        .next()
        .filter(|primary| !primary.is_empty())
        .unwrap_or(tag);

    Some(ParsedAcceptLanguage {
        tag: supported_tag,
        quality,
        position,
    })
}

fn parse_quality_value(value: &str) -> Option<u16> {
    let quality: f32 = value.parse().ok()?;
    if !(0.0..=1.0).contains(&quality) {
        return None;
    }
    Some((quality * 1000.0).round() as u16)
}

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::translation_prompt;

    use super::{
        default_translation_fallback_language, detect_browser_translation_language,
        translation_service,
    };

    #[test]
    fn translation_service_reports_supported_languages() {
        let llm = crate::llm::Llm {
            reqwest: reqwest::Client::new(),
            api_key: "test".to_string(),
            models: vec!["test-model".to_string()],
        };
        let service = translation_service(&llm);

        assert!(service
            .supported_languages()
            .iter()
            .any(|language| language.code == "pt"));
    }

    #[test]
    fn translation_prompt_version_is_versioned() {
        assert_eq!(translation_prompt().version, 1);
    }

    #[test]
    fn detect_browser_translation_language_prefers_highest_quality_supported_language() {
        let detected =
            detect_browser_translation_language(Some("fr-CA,pt-BR;q=0.9,en-US;q=0.8,de;q=0.7"))
                .unwrap();

        assert_eq!(detected.code, "fr");
    }

    #[test]
    fn detect_browser_translation_language_skips_unsupported_entries() {
        let detected =
            detect_browser_translation_language(Some("zh-CN,ja;q=0.8,pt-BR;q=0.6")).unwrap();

        assert_eq!(detected.code, "pt");
    }

    #[test]
    fn detect_browser_translation_language_rejects_wildcards_and_zero_quality_values() {
        let detected = detect_browser_translation_language(Some("*;q=1.0,en;q=0"));

        assert!(detected.is_none());
    }

    #[test]
    fn default_translation_fallback_language_uses_english_for_non_english_sources() {
        let source =
            crate::llm::prompt_registry::find_supported_translation_language("pt").unwrap();

        let fallback = default_translation_fallback_language(source);

        assert_eq!(fallback.code, "en");
    }

    #[test]
    fn default_translation_fallback_language_preserves_english_sources() {
        let source =
            crate::llm::prompt_registry::find_supported_translation_language("en").unwrap();

        let fallback = default_translation_fallback_language(source);

        assert_eq!(fallback.code, "en");
    }
}
