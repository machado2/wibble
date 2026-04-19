use serde_json::Value;

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;
use crate::llm::prompt_registry::{
    find_supported_translation_language, supported_translation_languages, translation_prompt,
    SupportedTranslationLanguage,
};
use crate::llm::{function_definition, Llm, Message};

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

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::translation_prompt;

    use super::translation_service;

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
}
