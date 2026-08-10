#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PromptDefinition {
    pub key: &'static str,
    pub version: i32,
    pub body: &'static str,
}

const ARTICLE_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "article_generation",
    version: 2,
    body: include_str!("../../prompts/system_article.txt"),
};

const RESEARCH_ARTICLE_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "research_article_generation",
    version: 1,
    body: include_str!("../../prompts/system_article_research.txt"),
};

const PLACEHOLDER_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "placeholder_generation",
    version: 3,
    body: include_str!("../../prompts/system_with_placeholders.txt"),
};

const IMAGE_BRIEF_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "image_brief_generation",
    version: 1,
    body: include_str!("../../prompts/illustrator.txt"),
};

const EDIT_REWRITE_PROMPT: PromptDefinition = PromptDefinition {
    key: "edit_rewrite",
    version: 1,
    body: include_str!("../../prompts/edit_rewrite.txt"),
};

pub fn article_generation_prompt() -> PromptDefinition {
    ARTICLE_GENERATION_PROMPT
}

pub fn placeholder_generation_prompt() -> PromptDefinition {
    PLACEHOLDER_GENERATION_PROMPT
}

pub fn research_article_generation_prompt() -> PromptDefinition {
    RESEARCH_ARTICLE_GENERATION_PROMPT
}

pub fn image_brief_generation_prompt() -> PromptDefinition {
    IMAGE_BRIEF_GENERATION_PROMPT
}

pub fn edit_rewrite_prompt() -> PromptDefinition {
    EDIT_REWRITE_PROMPT
}

#[cfg(test)]
mod tests {
    use super::{
        article_generation_prompt, edit_rewrite_prompt, image_brief_generation_prompt,
        placeholder_generation_prompt, research_article_generation_prompt,
    };

    #[test]
    fn prompt_definitions_expose_stable_versions() {
        assert_eq!(article_generation_prompt().version, 2);
        assert_eq!(research_article_generation_prompt().version, 1);
        assert_eq!(edit_rewrite_prompt().version, 1);
    }

    #[test]
    fn article_generation_prompt_contains_core_article_contract() {
        let prompt = article_generation_prompt().body;

        assert!(prompt.contains("The text is part of a game's fictional world"));
        assert!(prompt.contains("You never break the player immersion"));
    }

    #[test]
    fn research_generation_prompt_contains_source_contract() {
        let prompt = research_article_generation_prompt().body;

        assert!(prompt.contains("internal research file"));
        assert!(prompt.contains("Do not invent citations"));
        assert!(prompt.contains("stay general instead of fabricating specifics"));
    }

    #[test]
    fn placeholder_prompt_contains_generated_image_contract() {
        let prompt = placeholder_generation_prompt().body;

        assert!(prompt.contains("The only XML tag allowed is the <GeneratedImage>."));
        assert!(prompt.contains("Include images in the content"));
        assert!(prompt.contains("Don't use the markdown image syntax"));
    }

    #[test]
    fn image_brief_prompt_contains_line_format_contract() {
        let prompt = image_brief_generation_prompt().body;

        assert!(prompt.contains("Each line must be: caption; detailed description"));
        assert!(prompt.contains("One image per line."));
        assert!(prompt.contains("Do not add bullets, numbering, commentary"));
    }

    #[test]
    fn edit_rewrite_prompt_contains_preview_contract() {
        let prompt = edit_rewrite_prompt().body;

        assert!(prompt.contains("return a full revised markdown article"));
        assert!(prompt.contains("keep the number of Markdown image tags unchanged"));
        assert!(prompt.contains("Return only through the provided tool."));
    }
}
