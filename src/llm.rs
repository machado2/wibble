#![allow(dead_code)]

use std::env;

use serde_json::json;
use serde_json::Value;
use tracing::{debug, trace};

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;

pub mod article_generator;
mod article_text_writer;
mod function_definition;
pub mod translate;

const URL: &str = "https://openrouter.ai/api/v1/chat/completions";

#[derive(Debug, Clone)]
pub struct Llm {
    reqwest: reqwest::Client,
    api_key: String,
    pub models: Vec<String>,
}

#[derive(Debug)]
pub enum Message {
    System(String),
    User(String),
    Assistant(String),
}

impl Message {
    pub fn to_json(&self) -> Value {
        match self {
            Message::System(content) => json!({ "role": "system", "content": content }),
            Message::User(content) => json!({ "role": "user", "content": content }),
            Message::Assistant(content) => json!({ "role": "assistant", "content": content }),
        }
    }
}

impl Llm {
    pub fn init() -> Self {
        let api_key = env::var("OPENROUTER_API_KEY").expect("OPENROUTER_API_KEY must be set");
        let reqwest = reqwest::Client::new();
        let model = env::var("LANGUAGE_MODEL").expect("LANGUAGE_MODEL must be set");
        let models = model.split(',').map(|s| s.to_string()).collect();
        Self {
            reqwest,
            api_key,
            models,
        }
    }

    async fn post(&self, body: &Value) -> Result<Value, Error> {
        trace!(body = ?body, "Sending request");
        let resp = self
            .reqwest
            .post(URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to send request: {}", e)))?
            .text()
            .await
            .map_err(|e| Error::Llm(format!("Failed to read response: {}", e)))?;
        trace!(response = ?resp, "Received response");
        // strip any trailing text like <|im_end|> or similar
        let resp_text = resp.trim_end_matches(|c| c != '}' && c != ']');
        let resp: Value = serde_json::from_str(resp_text).map_err(|e| {
            Error::Llm(format!(
                "Failed to parse response: {}\nResponse: {}",
                e, resp_text
            ))
        })?;
        Ok(resp)
    }

    pub async fn request_tool(
        &self,
        tool: FunctionDefinition,
        messages: Vec<Message>,
        model: &str,
    ) -> Result<String, Error> {
        let messages: Vec<Value> = messages.iter().map(|m| m.to_json()).collect();
        let tool = tool.to_function_object();
        let req = json!({
            "model": model,
            "messages": messages,
            "temperature": 1f32,
            "frequency_penalty": 1.0f32,
            "stop": ["<|im_end|>"],
            "tools": [{
                "type": "function",
                "function": tool
            }]
        });
        let resp = self.post(&req).await?;

        let response_message = &resp["choices"][0]["message"];
        let finish_reason = &resp["choices"][0]["finish_reason"];
        debug!(?finish_reason, "Finish reason");
        if let Some(content) = response_message["content"].as_str() {
            Ok(content.to_string())
        } else if let Some(arguments) =
            response_message["tool_calls"][0]["function"]["arguments"].as_str()
        {
            Ok(arguments.to_string())
        } else {
            Err(Error::Llm("Tool response missing content".into()))
        }
    }

    pub async fn request_chat(&self, messages: Vec<Message>, model: &str) -> Result<String, Error> {
        let messages: Vec<Value> = messages.iter().map(|m| m.to_json()).collect();
        let req = json!({
            "model": model,
            "messages": messages,
            "stop": ["<|im_end|>", "<|eot_id|>"],
            // "frequency_penalty": 1f32,
            "repetition_penalty": 1f32,
            "temperature": 1f32,
            "max_tokens": 16000,
            "top_p": 0.3f32,
        });
        let resp = self.post(&req).await?;
        let response_message = &resp["choices"][0]["message"];
        let finish_reason = &resp["choices"][0]["finish_reason"];
        let prompt_tokens = &resp["usage"]["prompt_tokens"];
        let completion_tokens = &resp["usage"]["completion_tokens"];
        let total_tokens = &resp["usage"]["total_tokens"];
        debug!(
            ?finish_reason,
            ?prompt_tokens,
            ?completion_tokens,
            ?total_tokens,
            "LLM usage"
        );
        if let Some(content) = response_message["content"].as_str() {
            Ok(content.to_string())
        } else {
            Err(Error::Llm("Chat response missing content".into()))
        }
    }
}
