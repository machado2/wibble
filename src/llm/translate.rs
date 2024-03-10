use serde_json::Value;

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;
use crate::llm::{function_definition, Llm, Message};

pub trait Translate {
    async fn translate(&self, text: &str, target_language: &str) -> Result<String, Error>;
}

fn send_translated_text() -> FunctionDefinition {
    let mut f = function_definition::def_function("send_translated_text", "Send translated text");
    f.parameters.add_str("text", true, "Translated text");
    f
}

impl Translate for Llm {
    async fn translate(&self, text: &str, target_language: &str) -> Result<String, Error> {
        let messages = vec![
            Message::System("You are a translator.".to_string()),
            Message::User(format!(
                "Translate the following text to {}:\n\n{}",
                target_language, text
            )),
        ];
        let translation = self
            .request_tool(send_translated_text(), messages, &self.models[0])
            .await?;
        let value: Value = serde_json::from_str(&translation)
            .map_err(|e| Error::Llm(format!("Failed to parse translation response: {}", e)))?;
        Ok(value["text"]
            .as_str()
            .ok_or(Error::Llm("Translation response missing text".into()))?
            .to_string())
    }
}
