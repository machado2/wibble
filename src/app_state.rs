use std::env;
use std::sync::Arc;

use bustdir::BustDir;
use sea_orm::{Database, DatabaseConnection};
use tera::Tera;

use crate::image_generator::ai_horde::AiHordeImageGenerator;
use crate::image_generator::fallback::FallbackImageGenerator;
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::stability::StabilityImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::image_generator::retrying::RetryingImageGenerator;
use crate::image_generator::huggingface::HuggingFaceImageGenerator;
use crate::llm::Llm;
use crate::tasklist::TaskList;

async fn connect_database() -> DatabaseConnection {
    let connection = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Database::connect(connection)
        .await
        .expect("Failed to connect to database")
}

impl AppState {
    pub async fn init() -> Self {
        let image_mode = env::var("IMAGE_MODE").unwrap_or(String::from(""));
        let db = connect_database().await;
        let task_list = TaskList::default();
        let tera = Tera::new("templates/**/*").expect("Failed to load templates");
        let llm = Llm::init();
        println!("Image mode: {}", image_mode);
        let image_generator: Arc<dyn ImageGenerator> = if image_mode == "sd3" {
            println!("Using SD3");
            Arc::new(FallbackImageGenerator::new(
                StabilityImageGenerator::new(),
                AiHordeImageGenerator::new(),
            ))
        } else if image_mode == "horde" {
            println!("Using Horde");
            Arc::new(RetryingImageGenerator::new(
                AiHordeImageGenerator::new()
            ))
        } else if image_mode == "huggingface" {
            println!("Using Hugging Face");
            Arc::new(HuggingFaceImageGenerator::new())
        } else {
            println!("Using Replicate");
            Arc::new(RetryingImageGenerator::new(
                ReplicateImageGenerator::new()
            ))
        };
        Self {
            db,
            task_list,
            tera,
            llm,
            image_generator: image_generator,
            bust_dir: BustDir::new("static").expect("Failed to build bust dir"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub task_list: TaskList,
    pub tera: Tera,
    pub llm: Llm,
    pub image_generator: Arc<dyn ImageGenerator>,
    pub bust_dir: BustDir,
}
