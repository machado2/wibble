use crate::llm::prompt_registry::{
    article_generation_prompt, image_brief_generation_prompt, placeholder_generation_prompt,
    research_article_generation_prompt,
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

pub fn build_research_article_messages(instructions: &str) -> Vec<Message> {
    build_messages(
        research_article_generation_prompt().body,
        None,
        instructions,
    )
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

#[cfg(test)]
mod tests {
    use crate::llm::prompt_registry::{
        article_generation_prompt, image_brief_generation_prompt, placeholder_generation_prompt,
        research_article_generation_prompt,
    };
    use crate::llm::Message;

    use super::{
        build_article_messages, build_illustrator_messages, build_placeholder_messages,
        build_research_article_messages,
    };

    #[test]
    fn build_article_messages_uses_registered_prompt_and_user_input() {
        let messages = build_article_messages("Write about the audit");

        assert!(matches!(
            &messages[0],
            Message::System(body) if body == article_generation_prompt().body
        ));
        assert!(matches!(
            &messages[1],
            Message::User(body) if body == "Write about the audit"
        ));
    }

    #[test]
    fn build_placeholder_messages_preserves_example_order_before_prompt() {
        let messages = build_placeholder_messages(
            Some(vec![
                ("Prompt one".to_string(), "# Article one".to_string()),
                ("Prompt two".to_string(), "# Article two".to_string()),
            ]),
            "Main request",
        );

        assert!(matches!(
            &messages[0],
            Message::System(body) if body == placeholder_generation_prompt().body
        ));
        assert!(matches!(&messages[1], Message::User(body) if body == "Prompt one"));
        assert!(matches!(&messages[2], Message::Assistant(body) if body == "# Article one"));
        assert!(matches!(&messages[3], Message::User(body) if body == "Prompt two"));
        assert!(matches!(&messages[4], Message::Assistant(body) if body == "# Article two"));
        assert!(matches!(&messages[5], Message::User(body) if body == "Main request"));
    }

    #[test]
    fn build_illustrator_messages_uses_registered_prompt() {
        let messages = build_illustrator_messages("Full article body");

        assert!(matches!(
            &messages[0],
            Message::System(body) if body == image_brief_generation_prompt().body
        ));
        assert!(matches!(
            &messages[1],
            Message::User(body) if body == "Full article body"
        ));
    }

    #[test]
    fn build_research_article_messages_uses_research_prompt() {
        let messages = build_research_article_messages("Research-backed request");

        assert!(matches!(
            &messages[0],
            Message::System(body) if body == research_article_generation_prompt().body
        ));
        assert!(matches!(
            &messages[1],
            Message::User(body) if body == "Research-backed request"
        ));
    }
}
