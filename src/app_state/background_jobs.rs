use std::env;

use sea_orm::{ConnectionTrait, DatabaseConnection};

use crate::hot_score::update_hot_score_statement;
use crate::image_jobs;
use crate::translation_jobs;

use super::AppState;

pub fn bootstrap_background_jobs(state: AppState) {
    spawn_hot_score_update_loop(state.db.clone());
    image_jobs::spawn_resume_loop(state.clone());
    translation_jobs::spawn_resume_loop(state);
}

fn spawn_hot_score_update_loop(db: DatabaseConnection) {
    let interval_seconds = hot_score_update_interval_seconds();

    tokio::spawn(async move {
        let mut interval =
            tokio::time::interval(tokio::time::Duration::from_secs(interval_seconds));
        loop {
            interval.tick().await;
            let stmt = update_hot_score_statement();
            if let Err(err) = db.execute(stmt).await {
                eprintln!("Error updating hot_score: {}", err);
            }
        }
    });
}

fn hot_score_update_interval_seconds() -> u64 {
    env::var("HOT_SCORE_UPDATE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(300)
}
