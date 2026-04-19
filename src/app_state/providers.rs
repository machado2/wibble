use std::env;
use std::sync::Arc;

use bustdir::BustDir;
use tera::Tera;

use crate::error::Error;
use crate::image_generator::ai_horde::AiHordeImageGenerator;
use crate::image_generator::huggingface::HuggingFaceImageGenerator;
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::stability::StabilityImageGenerator;
use crate::image_generator::ImageGenerator;

use super::runtime::RuntimeLimits;

pub struct ImageProviders {
    pub requested_mode: String,
    pub name: String,
    pub generator: Arc<dyn ImageGenerator>,
    pub replicate: Option<Arc<ReplicateImageGenerator>>,
}

pub fn init_templates() -> Result<Tera, Error> {
    Tera::new("templates/**/*")
        .map_err(|e| Error::Template(tera::Error::msg(format!("Failed to load templates: {}", e))))
}

pub fn detect_template_auto_reload() -> bool {
    env::var("TEMPLATE_AUTO_RELOAD")
        .ok()
        .map(|value| parse_bool_flag(&value))
        .unwrap_or(cfg!(debug_assertions))
}

pub fn read_runtime_limits() -> RuntimeLimits {
    RuntimeLimits {
        max_concurrent_article_generations: read_positive_usize(
            "MAX_CONCURRENT_ARTICLE_GENERATIONS",
            1,
        ),
        dead_link_recovery_max_per_day: read_positive_usize("DEAD_LINK_RECOVERY_MAX_PER_DAY", 5),
    }
}

pub fn build_image_providers() -> ImageProviders {
    let image_mode = env::var("IMAGE_MODE").unwrap_or_default();

    if image_mode == "sd3" {
        println!("Using SD3");
        return ImageProviders {
            requested_mode: image_mode,
            name: "sd3".to_string(),
            generator: Arc::new(StabilityImageGenerator::new()),
            replicate: None,
        };
    }

    if image_mode == "horde" {
        println!("Using Horde");
        return ImageProviders {
            requested_mode: image_mode,
            name: "horde".to_string(),
            generator: Arc::new(AiHordeImageGenerator::new()),
            replicate: None,
        };
    }

    if image_mode == "huggingface" {
        println!("Using Hugging Face");
        return ImageProviders {
            requested_mode: image_mode,
            name: "huggingface".to_string(),
            generator: Arc::new(HuggingFaceImageGenerator::new()),
            replicate: None,
        };
    }

    println!("Using Replicate");
    let replicate = Arc::new(ReplicateImageGenerator::new());
    ImageProviders {
        requested_mode: image_mode,
        name: "replicate".to_string(),
        generator: replicate.clone() as Arc<dyn ImageGenerator>,
        replicate: Some(replicate),
    }
}

pub fn build_bust_dir() -> Result<BustDir, Error> {
    BustDir::new("static").map_err(|e| Error::Storage(format!("Failed to build bust dir: {}", e)))
}

pub fn log_startup_configuration(
    requested_image_mode: &str,
    image_provider_name: &str,
    runtime_limits: RuntimeLimits,
    schema_compatibility_mode: &str,
) {
    let requested_image_mode = if requested_image_mode.is_empty() {
        "<default>"
    } else {
        requested_image_mode
    };
    println!("IMAGE_MODE={}", requested_image_mode);
    println!("Image provider: {}", image_provider_name);
    println!(
        "MAX_CONCURRENT_ARTICLE_GENERATIONS={}",
        runtime_limits.max_concurrent_article_generations
    );
    println!(
        "DEAD_LINK_RECOVERY_MAX_PER_DAY={}",
        runtime_limits.dead_link_recovery_max_per_day
    );
    println!(
        "STARTUP_SCHEMA_COMPATIBILITY_MODE={}",
        schema_compatibility_mode
    );
}

pub fn log_static_dir_diagnostics() {
    match std::env::current_dir() {
        Ok(cwd) => println!("CWD = {:?}", cwd),
        Err(err) => println!("Failed to get CWD: {}", err),
    }

    match std::fs::metadata("static") {
        Ok(metadata) => {
            println!("static exists: is_dir={}", metadata.is_dir());
            if metadata.is_dir() {
                match std::fs::read_dir("static") {
                    Ok(entries) => {
                        for (index, entry) in entries.take(5).enumerate() {
                            match entry {
                                Ok(entry) => {
                                    println!("static entry {}: {:?}", index, entry.path());
                                }
                                Err(err) => {
                                    println!("static read_dir entry error: {}", err);
                                }
                            }
                        }
                    }
                    Err(err) => println!("Failed to read static dir: {}", err),
                }
            }
        }
        Err(err) => println!("static metadata error: {}", err),
    }
}

fn parse_bool_flag(value: &str) -> bool {
    let value = value.trim().to_lowercase();
    matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on")
}

fn read_positive_usize(var_name: &str, default: usize) -> usize {
    env::var(var_name)
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::{parse_bool_flag, read_positive_usize};

    #[test]
    fn parse_bool_flag_accepts_expected_truthy_values() {
        for value in ["1", "true", "TRUE", " yes ", "y", "on"] {
            assert!(parse_bool_flag(value), "expected truthy: {value}");
        }
    }

    #[test]
    fn parse_bool_flag_rejects_other_values() {
        for value in ["", "0", "false", "no", "off", "maybe"] {
            assert!(!parse_bool_flag(value), "expected falsey: {value}");
        }
    }

    #[test]
    fn read_positive_usize_falls_back_for_invalid_values() {
        assert_eq!(read_positive_usize("__WIBBLE_TEST_MISSING__", 7), 7);
    }
}
