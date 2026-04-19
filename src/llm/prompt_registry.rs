#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PromptDefinition {
    pub key: &'static str,
    pub version: i32,
    pub body: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SupportedTranslationLanguage {
    pub code: &'static str,
    pub name: &'static str,
    pub aliases: &'static [&'static str],
}

const ARTICLE_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "article_generation",
    version: 1,
    body: include_str!("../../prompts/system_article.txt"),
};

const RESEARCH_ARTICLE_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "research_article_generation",
    version: 1,
    body: include_str!("../../prompts/system_article_research.txt"),
};

const PLACEHOLDER_GENERATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "placeholder_generation",
    version: 2,
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

const TRANSLATION_PROMPT: PromptDefinition = PromptDefinition {
    key: "translation",
    version: 1,
    body: include_str!("../../prompts/translation_system.txt"),
};

const SUPPORTED_TRANSLATION_LANGUAGES: &[SupportedTranslationLanguage] = &[
    SupportedTranslationLanguage {
        code: "en",
        name: "English",
        aliases: &["english"],
    },
    SupportedTranslationLanguage {
        code: "pt",
        name: "Portuguese",
        aliases: &["portuguese", "portuguese (brazil)", "brazilian portuguese"],
    },
    SupportedTranslationLanguage {
        code: "es",
        name: "Spanish",
        aliases: &["spanish"],
    },
    SupportedTranslationLanguage {
        code: "fr",
        name: "French",
        aliases: &["french"],
    },
    SupportedTranslationLanguage {
        code: "de",
        name: "German",
        aliases: &["german"],
    },
    SupportedTranslationLanguage {
        code: "it",
        name: "Italian",
        aliases: &["italian"],
    },
];

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

pub fn translation_prompt() -> PromptDefinition {
    TRANSLATION_PROMPT
}

pub fn supported_translation_languages() -> &'static [SupportedTranslationLanguage] {
    SUPPORTED_TRANSLATION_LANGUAGES
}

pub fn find_supported_translation_language(value: &str) -> Option<SupportedTranslationLanguage> {
    let normalized = value.trim().to_ascii_lowercase();
    SUPPORTED_TRANSLATION_LANGUAGES
        .iter()
        .copied()
        .find(|language| {
            language.code == normalized
                || language.name.eq_ignore_ascii_case(&normalized)
                || language
                    .aliases
                    .iter()
                    .any(|alias| alias.eq_ignore_ascii_case(&normalized))
        })
}

#[cfg(test)]
mod tests {
    use super::{
        article_generation_prompt, edit_rewrite_prompt, find_supported_translation_language,
        image_brief_generation_prompt, placeholder_generation_prompt,
        research_article_generation_prompt, translation_prompt,
    };

    #[test]
    fn prompt_definitions_expose_stable_versions() {
        assert_eq!(article_generation_prompt().version, 1);
        assert_eq!(research_article_generation_prompt().version, 1);
        assert_eq!(edit_rewrite_prompt().version, 1);
        assert_eq!(translation_prompt().version, 1);
    }

    #[test]
    fn article_generation_prompt_contains_core_article_contract() {
        let prompt = article_generation_prompt().body;

        assert!(prompt.contains("The first line must be the headline"));
        assert!(prompt.contains("Reply with the article only, in Markdown."));
        assert!(prompt.contains("Do not add disclaimers"));
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

        assert!(prompt.contains("The only XML tag allowed is <GeneratedImage>."));
        assert!(prompt.contains("Include 3 to 4 <GeneratedImage> tags"));
        assert!(prompt.contains("Do not use Markdown image syntax."));
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

    #[test]
    fn translation_prompt_contains_markdown_preservation_contract() {
        let prompt = translation_prompt().body;

        assert!(prompt.contains("Keep markdown structure intact."));
        assert!(prompt.contains("Translate only the user-provided text."));
        assert!(prompt.contains("Return only the translated text through the tool."));
    }

    #[test]
    fn translation_language_lookup_accepts_codes_and_aliases() {
        assert_eq!(
            find_supported_translation_language("pt").unwrap().name,
            "Portuguese"
        );
        assert_eq!(
            find_supported_translation_language("Brazilian Portuguese")
                .unwrap()
                .code,
            "pt"
        );
    }

    #[test]
    fn translation_language_lookup_rejects_unknown_values() {
        assert!(find_supported_translation_language("klingon").is_none());
    }
}
