use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use bustdir::BustDir;
use sea_orm::DatabaseConnection;
use tera::Tera;
use tokio::sync::{Mutex, Semaphore};

use crate::auth::JwksClient;
use crate::error::Error;
use crate::image_generator::replicate::ReplicateImageGenerator;
use crate::image_generator::ImageGenerator;
use crate::llm::Llm;
use crate::rate_limit::RateLimitState;
use crate::tasklist::TaskList;

mod background_jobs;
mod db;
mod providers;
mod runtime;
mod schema_compat;

use background_jobs::bootstrap_background_jobs;
use db::connect_database;
use providers::{
    build_bust_dir, build_image_providers, detect_template_auto_reload, init_templates,
    log_startup_configuration, log_static_dir_diagnostics, read_runtime_limits,
};
use runtime::build_runtime_state;
use schema_compat::{
    apply_startup_schema_compatibility, startup_schema_compatibility_mode, validate_required_schema,
};

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

    pub async fn init() -> Result<Self, Error> {
        let db = connect_database().await?;
        apply_startup_schema_compatibility(&db).await;
        validate_required_schema(&db).await?;

        let runtime_limits = read_runtime_limits();
        let jwks_client = JwksClient::new();
        let task_list = TaskList::default();
        let tera = init_templates()?;
        let template_auto_reload = detect_template_auto_reload();
        let llm = Llm::init();
        let rate_limit_state = RateLimitState::new();
        let image_providers = build_image_providers();
        let runtime_state = build_runtime_state(tera, template_auto_reload, runtime_limits);

        log_startup_configuration(
            &image_providers.requested_mode,
            &image_providers.name,
            runtime_limits,
            startup_schema_compatibility_mode(),
        );
        log_static_dir_diagnostics();

        let state = Self {
            db,
            task_list,
            tera: runtime_state.tera,
            llm,
            image_generator: image_providers.generator,
            image_generator_name: image_providers.name,
            replicate_image_generator: image_providers.replicate,
            bust_dir: build_bust_dir()?,
            rate_limit_state,
            template_auto_reload: runtime_state.template_auto_reload,
            article_generation_semaphore: runtime_state.article_generation_semaphore,
            active_article_generations: runtime_state.active_article_generations,
            active_generation_ids: runtime_state.active_generation_ids,
            active_image_generation_ids: runtime_state.active_image_generation_ids,
            dead_link_recovery_max_per_day: runtime_state.dead_link_recovery_max_per_day,
            dead_link_recovery_timestamps: runtime_state.dead_link_recovery_timestamps,
            jwks_client,
        };

        bootstrap_background_jobs(state.clone());

        Ok(state)
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
