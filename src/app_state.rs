use std::env;
use std::sync::Arc;

use async_openai::config::OpenAIConfig;
use bustdir::BustDir;
use sea_orm::{Database, DatabaseConnection};
use sqlx::mysql::MySqlPoolOptions;
use sqlx::{MySql, Pool};
use tera::Tera;

use crate::image_generator::ai_horde::AiHordeImageGenerator;
use crate::image_generator::dalle::DallEImageGenerator;
use crate::image_generator::fallback::FallbackImageGenerator;
use crate::image_generator::stability::StabilityImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::image_generator::retrying::RetryingImageGenerator;
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
        let image_mode = env::var("IMAGE_MODE").expect("IMAGE_MODE must be set");
        let db = connect_database().await;
        let task_list = TaskList::default();
        let openai = async_openai::Client::new();
        let tera = Tera::new("templates/**/*").expect("Failed to load templates");
        let pool = MySqlPoolOptions::new()
            .max_connections(5)
            .connect(&env::var("DATABASE_URL").expect("DATABASE_URL must be set"))
            .await
            .expect("Failed to connect to database");
        let reqwest = reqwest::Client::new();
        let llm = Llm::init();
        let image_generator: Arc<dyn ImageGenerator> = if image_mode == "dalle" {
            Arc::new(FallbackImageGenerator::new(
                DallEImageGenerator::new(),
                AiHordeImageGenerator::new(),
            ))
        } else if image_mode == "sd3" {
            Arc::new(FallbackImageGenerator::new(
                StabilityImageGenerator::new(),
                AiHordeImageGenerator::new(),
            ))
        } else {
            Arc::new(RetryingImageGenerator::new(
                AiHordeImageGenerator::new()
            ))
        };
        Self {
            db,
            task_list,
            openai,
            reqwest,
            tera,
            pool,
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
    pub openai: async_openai::Client<OpenAIConfig>,
    pub tera: Tera,
    pub pool: Pool<MySql>,
    pub reqwest: reqwest::Client,
    pub llm: Llm,
    pub image_generator: Arc<dyn ImageGenerator>,
    pub bust_dir: BustDir,
}
