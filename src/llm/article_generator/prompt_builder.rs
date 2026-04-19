use crate::llm::Message;

const SYSTEM_MESSAGE_ARTICLE: &str = include_str!("../../../prompts/system_article.txt");
const SYSTEM_WITH_PLACEHOLDERS: &str =
    include_str!("../../../prompts/system_with_placeholders.txt");
const SYSTEM_MESSAGE_ILLUSTRATOR: &str = include_str!("../../../prompts/illustrator.txt");

pub fn build_article_messages(instructions: &str) -> Vec<Message> {
    build_messages(SYSTEM_MESSAGE_ARTICLE, None, instructions)
}

pub fn build_placeholder_messages(
    examples: Option<Vec<(String, String)>>,
    instructions: &str,
) -> Vec<Message> {
    build_messages(SYSTEM_WITH_PLACEHOLDERS, examples, instructions)
}

pub fn build_illustrator_messages(article: &str) -> Vec<Message> {
    build_messages(SYSTEM_MESSAGE_ILLUSTRATOR, None, article)
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
