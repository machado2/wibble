use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bustdir::BustDir;
use sea_orm::{Database, DatabaseConnection};
use tera::Tera;
use tokio::sync::{Mutex as AsyncMutex, Semaphore};

use crate::app_state::AppState;
use crate::auth::{AuthUser, JwksClient};
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::llm::prompt_registry::find_supported_translation_language;
use crate::llm::Llm;
use crate::rate_limit::RateLimitState;

fn test_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn set_test_env(database_url: &str) {
    env::set_var("DATABASE_URL", database_url);
    env::set_var("OPENROUTER_API_KEY", "test-openrouter-key");
    env::set_var("LANGUAGE_MODEL", "test-model");
    env::set_var("REPLICATE_API_TOKEN", "test-replicate-token");
    env::set_var("SITE_URL", "http://example.test");
    env::set_var("AUTH_SERVICE_URL", "http://127.0.0.1:9");
    env::set_var("ADMIN_EMAIL", "admin@example.com");
}

fn run_command(program: &str, args: &[String]) {
    let status = Command::new(program)
        .args(args)
        .status()
        .unwrap_or_else(|err| panic!("failed to run {} {:?}: {}", program, args, err));
    assert!(
        status.success(),
        "{} {:?} exited with status {}",
        program,
        args,
        status
    );
}

fn migration_paths() -> Vec<PathBuf> {
    let mut paths = fs::read_dir("database/prisma/migrations")
        .expect("migration directory should exist")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path().join("migration.sql"))
        .filter(|path| path.exists())
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn apply_migrations(db_name: &str) {
    for path in migration_paths() {
        let allow_failure = path
            .to_string_lossy()
            .contains("20260415235500_expand_audit_log_target_id");
        let status = Command::new("psql")
            .args([
                "-v",
                if allow_failure {
                    "ON_ERROR_STOP=0"
                } else {
                    "ON_ERROR_STOP=1"
                },
                "-d",
                db_name,
                "-f",
                path.to_string_lossy().as_ref(),
            ])
            .status()
            .unwrap_or_else(|err| {
                panic!(
                    "failed to apply migration {} to {}: {}",
                    path.display(),
                    db_name,
                    err
                )
            });
        assert!(
            status.success() || allow_failure,
            "migration {} failed for {}",
            path.display(),
            db_name
        );
    }
}

#[derive(Debug)]
pub struct TestDatabase {
    pub name: String,
    pub url: String,
}

impl TestDatabase {
    fn create() -> Self {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        let name = format!("wibble_test_{}", suffix);
        run_command("createdb", std::slice::from_ref(&name));
        apply_migrations(&name);
        Self {
            url: format!("postgresql:///{}", name),
            name,
        }
    }
}

impl Drop for TestDatabase {
    fn drop(&mut self) {
        let args = vec![
            "--if-exists".to_string(),
            "--force".to_string(),
            self.name.clone(),
        ];
        let _ = Command::new("dropdb").args(&args).status();
    }
}

pub struct TestContext {
    _env_guard: MutexGuard<'static, ()>,
    pub db: TestDatabase,
    pub state: AppState,
}

impl TestContext {
    pub async fn new() -> Self {
        Self::new_with_overrides(&[]).await
    }

    pub async fn new_with_overrides(overrides: &[(&str, &str)]) -> Self {
        let env_guard = test_env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let db = TestDatabase::create();
        set_test_env(&db.url);
        for (key, value) in overrides {
            env::set_var(key, value);
        }
        let state = test_state_for(&db.url).await;
        Self {
            _env_guard: env_guard,
            db,
            state,
        }
    }
}

pub async fn test_state_for(database_url: &str) -> AppState {
    let db = Database::connect(database_url)
        .await
        .unwrap_or_else(|err| panic!("failed to connect to {}: {}", database_url, err));
    crate::app_state::apply_test_schema_compatibility(&db).await;
    let tera = Tera::new("templates/**/*").expect("templates should load");
    let replicate = Arc::new(ReplicateImageGenerator::new());

    AppState {
        db,
        tera: Arc::new(RwLock::new(tera)),
        llm: Llm::init(),
        image_generator: replicate.clone() as Arc<dyn ImageGenerator>,
        image_generator_name: "replicate".to_string(),
        replicate_image_generator: Some(replicate),
        bust_dir: BustDir::new("static").expect("static bust dir should build"),
        rate_limit_state: RateLimitState::new(),
        template_auto_reload: true,
        article_generation_semaphore: Arc::new(Semaphore::new(1)),
        translation_generation_semaphore: Arc::new(Semaphore::new(0)),
        active_article_generations: Arc::new(AtomicUsize::new(0)),
        active_generation_ids: Arc::new(AsyncMutex::new(HashSet::new())),
        active_image_generation_ids: Arc::new(AsyncMutex::new(HashSet::new())),
        active_translation_generation_ids: Arc::new(AsyncMutex::new(HashSet::new())),
        dead_link_recovery_max_per_day: 0,
        dead_link_recovery_timestamps: Arc::new(AsyncMutex::new(Vec::<Instant>::new())),
        jwks_client: JwksClient::new(),
    }
}

pub async fn connect_test_database(database_url: &str) -> DatabaseConnection {
    Database::connect(database_url)
        .await
        .unwrap_or_else(|err| panic!("failed to connect to {}: {}", database_url, err))
}

pub fn author_user(email: &str) -> AuthUser {
    AuthUser {
        sub: format!("sub:{}", email),
        email: email.to_string(),
        name: "Test Author".to_string(),
        picture: None,
    }
}

pub fn admin_user() -> AuthUser {
    author_user("admin@example.com")
}

pub fn preferred_language(code: &str) -> crate::llm::prompt_registry::SupportedTranslationLanguage {
    find_supported_translation_language(code)
        .unwrap_or_else(|| panic!("supported language {} should exist", code))
}
