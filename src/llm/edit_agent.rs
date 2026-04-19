use serde_json::Value;

use crate::error::Error;
use crate::llm::function_definition::FunctionDefinition;
use crate::llm::prompt_registry::edit_rewrite_prompt;
use crate::llm::{function_definition, Llm, Message};

pub struct EditProposal {
    pub title: String,
    pub description: String,
    pub markdown: String,
    pub summary: String,
    pub prompt_version: i32,
}

fn submit_edit_proposal() -> FunctionDefinition {
    let mut f = function_definition::def_function(
        "submit_edit_proposal",
        "Return the revised article fields and a short editor summary",
    );
    f.parameters.add_str("title", true, "Revised article title");
    f.parameters
        .add_str("description", true, "Revised article description");
    f.parameters
        .add_str("markdown", true, "Revised article markdown body");
    f.parameters
        .add_str("summary", true, "Short editor-facing summary of the change");
    f
}

pub async fn generate_edit_proposal(
    llm: &Llm,
    model: &str,
    title: &str,
    description: &str,
    markdown: &str,
    change_request: &str,
) -> Result<EditProposal, Error> {
    let prompt = edit_rewrite_prompt();
    let messages = vec![
        Message::System(prompt.body.to_string()),
        Message::User(format!(
            "Current title:\n{}\n\nCurrent description:\n{}\n\nCurrent markdown:\n{}\n\nRequested change:\n{}",
            title, description, markdown, change_request
        )),
    ];
    let response = llm
        .request_tool(submit_edit_proposal(), messages, model)
        .await?;
    let value: Value = serde_json::from_str(&response)
        .map_err(|e| Error::Llm(format!("Failed to parse edit proposal response: {}", e)))?;

    Ok(EditProposal {
        title: required_field(&value, "title")?.trim().to_string(),
        description: required_field(&value, "description")?.trim().to_string(),
        markdown: required_field(&value, "markdown")?.trim().to_string(),
        summary: required_field(&value, "summary")?.trim().to_string(),
        prompt_version: prompt.version,
    })
}

fn required_field<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value[key]
        .as_str()
        .ok_or_else(|| Error::Llm(format!("Edit proposal missing {}", key)))
}
