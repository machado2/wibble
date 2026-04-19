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

pub fn image_brief_generation_prompt() -> PromptDefinition {
    IMAGE_BRIEF_GENERATION_PROMPT
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
        article_generation_prompt, find_supported_translation_language, translation_prompt,
    };

    #[test]
    fn prompt_definitions_expose_stable_versions() {
        assert_eq!(article_generation_prompt().version, 1);
        assert_eq!(translation_prompt().version, 1);
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
