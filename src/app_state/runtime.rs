use std::collections::HashSet;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;

use tera::Tera;
use tokio::sync::{Mutex, Semaphore};

#[derive(Clone, Copy, Debug)]
pub struct RuntimeLimits {
    pub max_concurrent_article_generations: usize,
    pub dead_link_recovery_max_per_day: usize,
}

pub struct RuntimeState {
    pub tera: Arc<RwLock<Tera>>,
    pub template_auto_reload: bool,
    pub article_generation_semaphore: Arc<Semaphore>,
    pub active_article_generations: Arc<AtomicUsize>,
    pub active_generation_ids: Arc<Mutex<HashSet<String>>>,
    pub active_image_generation_ids: Arc<Mutex<HashSet<String>>>,
    pub dead_link_recovery_max_per_day: usize,
    pub dead_link_recovery_timestamps: Arc<Mutex<Vec<Instant>>>,
}

pub fn build_runtime_state(
    tera: Tera,
    template_auto_reload: bool,
    limits: RuntimeLimits,
) -> RuntimeState {
    RuntimeState {
        tera: Arc::new(RwLock::new(tera)),
        template_auto_reload,
        article_generation_semaphore: Arc::new(Semaphore::new(
            limits.max_concurrent_article_generations,
        )),
        active_article_generations: Arc::new(AtomicUsize::new(0)),
        active_generation_ids: Arc::new(Mutex::new(HashSet::new())),
        active_image_generation_ids: Arc::new(Mutex::new(HashSet::new())),
        dead_link_recovery_max_per_day: limits.dead_link_recovery_max_per_day,
        dead_link_recovery_timestamps: Arc::new(Mutex::new(Vec::new())),
    }
}
