use std::env;
use std::sync::Arc;

use bustdir::BustDir;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement, DbBackend};
use tera::Tera;

use crate::image_generator::ai_horde::AiHordeImageGenerator;
use crate::image_generator::fallback::FallbackImageGenerator;
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::stability::StabilityImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::image_generator::retrying::RetryingImageGenerator;
use crate::image_generator::huggingface::HuggingFaceImageGenerator;
use crate::llm::Llm;
use crate::rate_limit::RateLimitState;
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
        let rate_limit_state = RateLimitState::new();
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
        // Add diagnostics to help identify missing/incorrect `static` directory in production.
        // This prints current working directory, metadata for "static" and up to 5 entries if it exists.
        let state = {
            match std::env::current_dir() {
                Ok(cwd) => println!("CWD = {:?}", cwd),
                Err(e) => println!("Failed to get CWD: {}", e),
            }
            match std::fs::metadata("static") {
                Ok(m) => {
                    println!("static exists: is_dir={}", m.is_dir());
                    if m.is_dir() {
                        match std::fs::read_dir("static") {
                            Ok(entries) => {
                                for (i, entry) in entries.take(5).enumerate() {
                                    match entry {
                                        Ok(e) => println!("static entry {}: {:?}", i, e.path()),
                                        Err(e) => println!("static read_dir entry error: {}", e),
                                    }
                                }
                            }
                            Err(e) => println!("Failed to read static dir: {}", e),
                        }
                    }
                }
                Err(e) => println!("static metadata error: {}", e),
            }

            Self {
                db,
                task_list,
                tera,
                llm,
                image_generator,
                bust_dir: BustDir::new("static").expect("Failed to build bust dir"),
                rate_limit_state,
            }
        };

        // Spawn a background task to periodically recompute `hot_score` in the DB.
        // Frequency can be configured with HOT_SCORE_UPDATE_SECONDS (default 300s).
        {
            let db_clone = state.db.clone();
            let secs = std::env::var("HOT_SCORE_UPDATE_SECONDS")
                .ok()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(300u64);

            tokio::spawn(async move {
                let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(secs));
                loop {
                    interval.tick().await;
                    // Mirror the previous formula: click_rate * 0.7 + (1 / age_hours) * 0.3
                    let sql = r#"
                    UPDATE content
                    SET hot_score = (
                        (CASE WHEN impression_count > 0
                            THEN (click_count::double precision / impression_count::double precision)
                            ELSE 0.0
                        END) * 0.7
                    )
                    + (
                        (1.0 / GREATEST(EXTRACT(EPOCH FROM (now() - created_at)) / 3600.0, 1.0))
                        * 0.3
                    )
                    WHERE generating = false AND flagged = false;
                    "#;
                    let stmt = Statement::from_string(DbBackend::Postgres, sql.to_string());
                    if let Err(e) = db_clone.execute(stmt).await {
                        eprintln!("Error updating hot_score: {}", e);
                    }
                }
            });
        }

        state
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
    pub rate_limit_state: RateLimitState,
}
