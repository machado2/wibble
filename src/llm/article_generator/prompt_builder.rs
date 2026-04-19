use crate::llm::prompt_registry::{
    article_generation_prompt, image_brief_generation_prompt, placeholder_generation_prompt,
};
use crate::llm::Message;

pub fn build_article_messages(instructions: &str) -> Vec<Message> {
    build_messages(article_generation_prompt().body, None, instructions)
}

pub fn build_placeholder_messages(
    examples: Option<Vec<(String, String)>>,
    instructions: &str,
) -> Vec<Message> {
    build_messages(placeholder_generation_prompt().body, examples, instructions)
}

pub fn build_illustrator_messages(article: &str) -> Vec<Message> {
    build_messages(image_brief_generation_prompt().body, None, article)
}

fn build_messages(
    system_message: &str,
    examples: Option<Vec<(String, String)>>,
    instructions: &str,
) -> Vec<Message> {
    let mut messages = Vec::<Message>::new();
    messages.push(Message::System(system_message.to_string()));

    if let Some(examples) = examples {
        for (prompt, article) in examples {
            messages.push(Message::User(prompt));
            messages.push(Message::Assistant(article));
        }
    }

    messages.push(Message::User(instructions.to_string()));
    messages
}
