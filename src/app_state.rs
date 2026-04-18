use std::collections::HashSet;
use std::env;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use bustdir::BustDir;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use tera::Tera;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;

use crate::auth::JwksClient;
use crate::hot_score::update_hot_score_statement;
use crate::image_generator::ai_horde::AiHordeImageGenerator;
use crate::image_generator::huggingface::HuggingFaceImageGenerator;
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::stability::StabilityImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::image_jobs;
use crate::llm::Llm;
use crate::rate_limit::RateLimitState;
use crate::tasklist::TaskList;

async fn connect_database() -> DatabaseConnection {
    let connection = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    Database::connect(connection)
        .await
        .expect("Failed to connect to database")
}

async fn ensure_async_image_job_columns(db: &DatabaseConnection) {
    let statements = [
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "status" VARCHAR(32) NOT NULL DEFAULT 'completed'"#,
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "last_error" TEXT"#,
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "generation_started_at" TIMESTAMP(6)"#,
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "generation_finished_at" TIMESTAMP(6)"#,
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "provider_job_id" VARCHAR(100)"#,
        r#"ALTER TABLE "public"."content_image"
           ADD COLUMN IF NOT EXISTS "provider_job_url" VARCHAR(1000)"#,
        r#"UPDATE "public"."content_image"
           SET "status" = 'completed'
           WHERE "status" IS NULL"#,
        r#"CREATE INDEX IF NOT EXISTS "content_image_status_created_at_idx"
           ON "public"."content_image"("status", "created_at")"#,
    ];

    for sql in statements {
        let stmt = Statement::from_string(DbBackend::Postgres, sql.to_string());
        if let Err(err) = db.execute(stmt).await {
            eprintln!("Error ensuring async image job columns: {}", err);
        }
    }
}

async fn ensure_auth_columns(db: &DatabaseConnection) {
    let statements = [
        r#"ALTER TABLE "public"."content"
           ADD COLUMN IF NOT EXISTS "author_email" VARCHAR(350)"#,
        r#"CREATE TABLE IF NOT EXISTS "public"."audit_log" (
            "id" VARCHAR(36) PRIMARY KEY,
            "user_email" VARCHAR(350) NOT NULL,
            "user_name" VARCHAR(500),
            "action" VARCHAR(100) NOT NULL,
            "target_type" VARCHAR(50) NOT NULL,
            "target_id" VARCHAR(500) NOT NULL,
            "details" TEXT,
            "created_at" TIMESTAMP(6) DEFAULT NOW()
        )"#,
        r#"ALTER TABLE "public"."audit_log"
           ALTER COLUMN "target_id" TYPE VARCHAR(500)"#,
        r#"CREATE INDEX IF NOT EXISTS "audit_log_created_at_idx"
           ON "public"."audit_log"("created_at")"#,
        r#"CREATE INDEX IF NOT EXISTS "audit_log_target_idx"
           ON "public"."audit_log"("target_type", "target_id")"#,
        r#"ALTER TABLE "public"."content"
           ADD COLUMN IF NOT EXISTS "published" BOOLEAN NOT NULL DEFAULT true"#,
        r#"ALTER TABLE "public"."content"
           ADD COLUMN IF NOT EXISTS "recovered_from_dead_link" BOOLEAN NOT NULL DEFAULT false"#,
    ];

    for sql in statements {
        let stmt = Statement::from_string(DbBackend::Postgres, sql.to_string());
        if let Err(err) = db.execute(stmt).await {
            eprintln!("Error ensuring auth columns: {}", err);
        }
    }
}

async fn ensure_comment_tables(db: &DatabaseConnection) {
    let statements = [
        r#"CREATE TABLE IF NOT EXISTS "public"."content_comment" (
            "id" VARCHAR(36) PRIMARY KEY,
            "content_id" VARCHAR(36) NOT NULL,
            "user_email" VARCHAR(350) NOT NULL,
            "user_name" VARCHAR(500) NOT NULL,
            "body" TEXT NOT NULL,
            "created_at" TIMESTAMP(6) DEFAULT NOW()
        )"#,
        r#"CREATE INDEX IF NOT EXISTS "idx_content_comment_content_created_at"
           ON "public"."content_comment"("content_id", "created_at")"#,
        r#"CREATE INDEX IF NOT EXISTS "idx_content_comment_user_created_at"
           ON "public"."content_comment"("user_email", "created_at")"#,
        r#"DO $$
        BEGIN
            IF NOT EXISTS (
                SELECT 1
                FROM pg_constraint
                WHERE conname = 'content_comment_content_id_fkey'
            ) THEN
                ALTER TABLE "public"."content_comment"
                ADD CONSTRAINT "content_comment_content_id_fkey"
                FOREIGN KEY ("content_id") REFERENCES "public"."content"("id")
                ON DELETE CASCADE ON UPDATE NO ACTION;
            END IF;
        END $$"#,
    ];

    for sql in statements {
        let stmt = Statement::from_string(DbBackend::Postgres, sql.to_string());
        if let Err(err) = db.execute(stmt).await {
            eprintln!("Error ensuring comment tables: {}", err);
        }
    }
}

impl AppState {
    pub async fn mark_generation_started(&self, article_id: &str) {
        self.active_generation_ids
            .lock()
            .await
            .insert(article_id.to_string());
    }

    pub async fn mark_generation_finished(&self, article_id: &str) {
        self.active_generation_ids.lock().await.remove(article_id);
    }

    pub async fn is_generation_active(&self, article_id: &str) -> bool {
        self.active_generation_ids.lock().await.contains(article_id)
    }

    pub async fn try_mark_image_generation_started(&self, image_id: &str) -> bool {
        self.active_image_generation_ids
            .lock()
            .await
            .insert(image_id.to_string())
    }

    pub async fn mark_image_generation_finished(&self, image_id: &str) {
        self.active_image_generation_ids
            .lock()
            .await
            .remove(image_id);
    }

    pub async fn is_image_generation_active(&self, image_id: &str) -> bool {
        self.active_image_generation_ids
            .lock()
            .await
            .contains(image_id)
    }

    pub async fn try_take_dead_link_recovery_slot(&self) -> bool {
        let mut timestamps = self.dead_link_recovery_timestamps.lock().await;
        let now = Instant::now();
        timestamps.retain(|t| now.duration_since(*t) < Duration::from_secs(60 * 60 * 24));
        if timestamps.len() >= self.dead_link_recovery_max_per_day {
            return false;
        }
        timestamps.push(now);
        true
    }

    pub async fn init() -> Self {
        let image_mode = env::var("IMAGE_MODE").unwrap_or(String::from(""));
        let db = connect_database().await;
        ensure_async_image_job_columns(&db).await;
        ensure_auth_columns(&db).await;
        ensure_comment_tables(&db).await;
        let jwks_client = JwksClient::new();
        let task_list = TaskList::default();
        let tera = Tera::new("templates/**/*").expect("Failed to load templates");
        let template_auto_reload = env::var("TEMPLATE_AUTO_RELOAD")
            .ok()
            .map(|val| {
                let val = val.trim().to_lowercase();
                matches!(val.as_str(), "1" | "true" | "yes" | "y" | "on")
            })
            .unwrap_or(cfg!(debug_assertions));
        let llm = Llm::init();
        let rate_limit_state = RateLimitState::new();
        let max_concurrent_article_generations = env::var("MAX_CONCURRENT_ARTICLE_GENERATIONS")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .filter(|v| *v > 0)
            .unwrap_or(1);
        let dead_link_recovery_max_per_day = env::var("DEAD_LINK_RECOVERY_MAX_PER_DAY")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(5);
        println!("Image mode: {}", image_mode);
        println!(
            "MAX_CONCURRENT_ARTICLE_GENERATIONS={}",
            max_concurrent_article_generations
        );
        println!(
            "DEAD_LINK_RECOVERY_MAX_PER_DAY={}",
            dead_link_recovery_max_per_day
        );
        let (image_generator_name, image_generator, replicate_image_generator): (
            String,
            Arc<dyn ImageGenerator>,
            Option<Arc<ReplicateImageGenerator>>,
        ) = if image_mode == "sd3" {
            println!("Using SD3");
            (
                "sd3".to_string(),
                Arc::new(StabilityImageGenerator::new()),
                None,
            )
        } else if image_mode == "horde" {
            println!("Using Horde");
            (
                "horde".to_string(),
                Arc::new(AiHordeImageGenerator::new()),
                None,
            )
        } else if image_mode == "huggingface" {
            println!("Using Hugging Face");
            (
                "huggingface".to_string(),
                Arc::new(HuggingFaceImageGenerator::new()),
                None,
            )
        } else {
            println!("Using Replicate");
            let replicate = Arc::new(ReplicateImageGenerator::new());
            (
                "replicate".to_string(),
                replicate.clone() as Arc<dyn ImageGenerator>,
                Some(replicate),
            )
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
                tera: Arc::new(RwLock::new(tera)),
                llm,
                image_generator,
                image_generator_name,
                replicate_image_generator,
                bust_dir: BustDir::new("static").expect("Failed to build bust dir"),
                rate_limit_state,
                template_auto_reload,
                article_generation_semaphore: Arc::new(Semaphore::new(
                    max_concurrent_article_generations,
                )),
                active_article_generations: Arc::new(AtomicUsize::new(0)),
                active_generation_ids: Arc::new(Mutex::new(HashSet::new())),
                active_image_generation_ids: Arc::new(Mutex::new(HashSet::new())),
                dead_link_recovery_max_per_day,
                dead_link_recovery_timestamps: Arc::new(Mutex::new(Vec::new())),
                jwks_client,
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
                    let stmt = update_hot_score_statement();
                    if let Err(e) = db_clone.execute(stmt).await {
                        eprintln!("Error updating hot_score: {}", e);
                    }
                }
            });
        }

        image_jobs::spawn_resume_loop(state.clone());

        state
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub task_list: TaskList,
    pub tera: Arc<RwLock<Tera>>,
    pub llm: Llm,
    pub image_generator: Arc<dyn ImageGenerator>,
    pub image_generator_name: String,
    pub replicate_image_generator: Option<Arc<ReplicateImageGenerator>>,
    pub bust_dir: BustDir,
    pub rate_limit_state: RateLimitState,
    pub template_auto_reload: bool,
    pub article_generation_semaphore: Arc<Semaphore>,
    pub active_article_generations: Arc<AtomicUsize>,
    pub active_generation_ids: Arc<Mutex<HashSet<String>>>,
    pub active_image_generation_ids: Arc<Mutex<HashSet<String>>>,
    pub dead_link_recovery_max_per_day: usize,
    pub dead_link_recovery_timestamps: Arc<Mutex<Vec<Instant>>>,
    pub jwks_client: JwksClient,
}
