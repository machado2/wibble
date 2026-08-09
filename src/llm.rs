#![allow(dead_code)]

use std::env;

use serde_json::json;
use serde_json::Value;
use tracing::{debug, trace};

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;

pub mod article_generator;
pub mod edit_agent;
mod function_definition;
pub mod prompt_registry;
pub mod translate;

const OPENAI_RESPONSES_URL: &str = "https://api.openai.com/v1/responses";
const OPENROUTER_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

fn first_nonempty_env(names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiKind {
    OpenAiResponses,
    ChatCompletions,
}

#[derive(Debug, Clone)]
pub struct Llm {
    reqwest: reqwest::Client,
    api_key: String,
    api_url: String,
    api_kind: ApiKind,
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
        let (api_key, api_url, api_kind) =
            if let Some(api_key) = first_nonempty_env(&["OPENAI_API_KEY"]) {
                let api_url = first_nonempty_env(&["OPENAI_API_URL"])
                    .unwrap_or_else(|| OPENAI_RESPONSES_URL.to_string());
                (api_key, api_url, ApiKind::OpenAiResponses)
            } else {
                let api_key = first_nonempty_env(&["OPENROUTER_API_KEY"])
                    .expect("OPENAI_API_KEY or OPENROUTER_API_KEY must be set");
                let api_url = first_nonempty_env(&["OPENROUTER_API_URL"])
                    .unwrap_or_else(|| OPENROUTER_URL.to_string());
                (api_key, api_url, ApiKind::ChatCompletions)
            };
        let reqwest = reqwest::Client::new();
        let model = first_nonempty_env(&["LANGUAGE_MODEL", "OPENAI_MODEL", "OPENROUTER_MODEL"])
            .expect("LANGUAGE_MODEL, OPENAI_MODEL, or OPENROUTER_MODEL must be set");
        let models = model
            .split(',')
            .map(str::trim)
            .filter(|model| !model.is_empty())
            .map(str::to_string)
            .collect();
        Self {
            reqwest,
            api_key,
            api_url,
            api_kind,
            models,
        }
    }

    async fn post(&self, body: &Value) -> Result<Value, Error> {
        trace!(body = ?body, "Sending request");
        let resp = self
            .reqwest
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Llm(format!("Failed to send request: {}", e)))?;
        let status = resp.status();
        let resp = resp
            .text()
            .await
            .map_err(|e| Error::Llm(format!("Failed to read response: {}", e)))?;
        if !status.is_success() {
            return Err(Error::Llm(format!(
                "Language model request failed with status {}: {}",
                status, resp
            )));
        }
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
        let messages: Vec<Value> = messages.iter().map(Message::to_json).collect();
        let tool = tool.to_function_object();
        let req = match self.api_kind {
            ApiKind::OpenAiResponses => json!({
                "model": model,
                "input": messages,
                "reasoning": { "effort": "none" },
                "tools": [{
                    "type": "function",
                    "name": tool["name"],
                    "description": tool["description"],
                    "parameters": tool["parameters"]
                }],
                "tool_choice": {
                    "type": "function",
                    "name": tool["name"]
                }
            }),
            ApiKind::ChatCompletions => json!({
                "model": model,
                "messages": messages,
                "temperature": 1f32,
                "frequency_penalty": 1.0f32,
                "stop": ["<|im_end|>"],
                "tools": [{
                    "type": "function",
                    "function": tool
                }]
            }),
        };
        let resp = self.post(&req).await?;
        match self.api_kind {
            ApiKind::OpenAiResponses => extract_responses_tool_output(&resp),
            ApiKind::ChatCompletions => {
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
        }
    }

    pub async fn request_chat(&self, messages: Vec<Message>, model: &str) -> Result<String, Error> {
        let messages: Vec<Value> = messages.iter().map(Message::to_json).collect();
        let req = match self.api_kind {
            ApiKind::OpenAiResponses => json!({
                "model": model,
                "input": messages,
                "reasoning": { "effort": "none" },
                "max_output_tokens": 16000
            }),
            ApiKind::ChatCompletions => json!({
                "model": model,
                "messages": messages,
                "stop": ["<|im_end|>", "<|eot_id|>"],
                "temperature": 1f32,
                "max_tokens": 16000,
                "top_p": 0.3f32,
            }),
        };
        let resp = self.post(&req).await?;
        match self.api_kind {
            ApiKind::OpenAiResponses => {
                let status = &resp["status"];
                let input_tokens = &resp["usage"]["input_tokens"];
                let output_tokens = &resp["usage"]["output_tokens"];
                let total_tokens = &resp["usage"]["total_tokens"];
                debug!(
                    ?status,
                    ?input_tokens,
                    ?output_tokens,
                    ?total_tokens,
                    "LLM usage"
                );
                extract_responses_text(&resp)
            }
            ApiKind::ChatCompletions => {
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
                    Err(Error::Llm(format!(
                        "Chat response missing content: {}",
                        resp
                    )))
                }
            }
        }
    }
}

fn responses_output_text(resp: &Value) -> Option<String> {
    let text = resp["output"]
        .as_array()?
        .iter()
        .filter(|item| item["type"] == "message")
        .filter_map(|item| item["content"].as_array())
        .flatten()
        .filter(|content| content["type"] == "output_text")
        .filter_map(|content| content["text"].as_str())
        .collect::<Vec<_>>()
        .join("");
    (!text.is_empty()).then_some(text)
}

fn extract_responses_text(resp: &Value) -> Result<String, Error> {
    responses_output_text(resp).ok_or_else(|| {
        Error::Llm(format!(
            "Responses API response missing output text: {}",
            resp
        ))
    })
}

fn extract_responses_tool_output(resp: &Value) -> Result<String, Error> {
    let arguments = resp["output"].as_array().and_then(|output| {
        output
            .iter()
            .find(|item| item["type"] == "function_call")
            .and_then(|item| item["arguments"].as_str())
    });
    arguments
        .map(str::to_string)
        .or_else(|| responses_output_text(resp))
        .ok_or_else(|| {
            Error::Llm(format!(
                "Responses API response missing tool output: {}",
                resp
            ))
        })
}

#[cfg(test)]
mod tests {
    use super::{extract_responses_text, extract_responses_tool_output};
    use serde_json::json;

    #[test]
    fn extracts_text_from_responses_output() {
        let response = json!({
            "output": [{
                "type": "message",
                "content": [
                    { "type": "output_text", "text": "Hello " },
                    { "type": "output_text", "text": "world" }
                ]
            }]
        });

        assert_eq!(extract_responses_text(&response).unwrap(), "Hello world");
    }

    #[test]
    fn extracts_function_arguments_from_responses_output() {
        let response = json!({
            "output": [{
                "type": "function_call",
                "name": "publish_article",
                "arguments": "{\"title\":\"News\"}"
            }]
        });

        assert_eq!(
            extract_responses_tool_output(&response).unwrap(),
            "{\"title\":\"News\"}"
        );
    }
}
