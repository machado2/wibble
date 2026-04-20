use std::env;
use std::time::Duration;

use crate::entities::translation_job;

use super::definitions::{
    TRANSLATION_JOB_STATUS_COMPLETED, TRANSLATION_JOB_STATUS_FAILED,
    TRANSLATION_JOB_STATUS_PROCESSING, TRANSLATION_JOB_STATUS_QUEUED,
};

pub(super) fn translation_resume_interval_seconds() -> u64 {
    env::var("TRANSLATION_RESUME_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(30)
}

pub(super) fn translation_resume_batch_size() -> u64 {
    env::var("TRANSLATION_RESUME_BATCH_SIZE")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(50)
}

fn translation_retry_base_seconds() -> u64 {
    env::var("TRANSLATION_RETRY_BASE_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(60)
}

fn translation_retry_max_seconds() -> u64 {
    env::var("TRANSLATION_RETRY_MAX_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(15 * 60)
}

pub(super) fn translation_retry_delay(fail_count: i32) -> Duration {
    let exponent = fail_count.saturating_sub(1).clamp(0, 4) as u32;
    let retry_seconds = translation_retry_base_seconds()
        .saturating_mul(1_u64 << exponent)
        .min(translation_retry_max_seconds());
    Duration::from_secs(retry_seconds)
}

pub(super) fn translation_retry_max_chrono_seconds() -> i64 {
    translation_retry_max_seconds() as i64
}

pub(super) fn now() -> chrono::NaiveDateTime {
    chrono::Utc::now().naive_local()
}

pub(super) fn should_requeue_job(
    job: &translation_job::Model,
    reference_time: chrono::NaiveDateTime,
) -> bool {
    match job.status.as_str() {
        TRANSLATION_JOB_STATUS_QUEUED | TRANSLATION_JOB_STATUS_PROCESSING => false,
        TRANSLATION_JOB_STATUS_FAILED => job
            .next_retry_at
            .is_none_or(|retry_at| retry_at <= reference_time),
        TRANSLATION_JOB_STATUS_COMPLETED => true,
        _ => true,
    }
}
