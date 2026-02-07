use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use serde_json::{json, Value};
use tokio::sync::Semaphore;
use tokio::time::{sleep, timeout};
use tracing::{event, Level};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
pub struct ReplicateImageGenerator {
    reqwest: reqwest::Client,
    api_token: String,
    api_url: String,
    api_base_url: String,
    poll_interval: Duration,
    poll_timeout: Duration,
    queue_timeout: Duration,
    semaphore: Arc<Semaphore>,
    inflight_predictions: Arc<AtomicUsize>,
}

#[derive(Debug)]
struct InflightGuard {
    inflight_predictions: Arc<AtomicUsize>,
}

impl InflightGuard {
    fn new(inflight_predictions: Arc<AtomicUsize>) -> Self {
        inflight_predictions.fetch_add(1, Ordering::SeqCst);
        Self {
            inflight_predictions,
        }
    }
}

impl Drop for InflightGuard {
    fn drop(&mut self) {
        self.inflight_predictions.fetch_sub(1, Ordering::SeqCst);
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(default)
}

fn env_usize(name: &str, default: usize) -> usize {
    env::var(name)
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn normalize_status(status: &str) -> String {
    status.trim().to_ascii_lowercase()
}

fn is_completed_status(status: &str) -> bool {
    let status = normalize_status(status);
    status == "succeeded" || status == "completed"
}

fn is_failed_status(status: &str) -> bool {
    let status = normalize_status(status);
    status == "failed" || status == "canceled" || status == "cancelled"
}

fn extract_output_url(output: Option<&Value>) -> Option<String> {
    let output = output?;
    match output {
        Value::String(url) => Some(url.clone()),
        Value::Array(items) => items.iter().find_map(|item| match item {
            Value::String(url) => Some(url.clone()),
            Value::Object(obj) => obj
                .get("url")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            _ => None,
        }),
        Value::Object(obj) => obj
            .get("url")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

impl ReplicateImageGenerator {
    pub(crate) fn new() -> Self {
        let api_token = env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");
        let api_base_url = env::var("REPLICATE_API_BASE_URL")
            .unwrap_or_else(|_| "https://api.replicate.com/v1".to_string());
        let api_url = env::var("REPLICATE_API_URL").unwrap_or_else(|_| {
            "https://api.replicate.com/v1/models/black-forest-labs/flux-schnell/predictions"
                .to_string()
        });
        let http_timeout = Duration::from_secs(env_u64("REPLICATE_HTTP_TIMEOUT_SECONDS", 30));
        let connect_timeout = Duration::from_secs(env_u64("REPLICATE_CONNECT_TIMEOUT_SECONDS", 10));
        let poll_timeout = Duration::from_secs(env_u64("REPLICATE_POLL_TIMEOUT_SECONDS", 180));
        let poll_interval = Duration::from_millis(env_u64("REPLICATE_POLL_INTERVAL_MS", 1_500));
        let queue_timeout = Duration::from_secs(env_u64("REPLICATE_QUEUE_TIMEOUT_SECONDS", 30));
        let max_concurrency = env_usize("REPLICATE_MAX_CONCURRENT_REQUESTS", 2);

        let reqwest = reqwest::Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(http_timeout)
            .build()
            .expect("Failed to build reqwest client for Replicate API");

        println!(
            "Replicate config: max_concurrency={}, http_timeout_secs={}, poll_timeout_secs={}",
            max_concurrency,
            http_timeout.as_secs(),
            poll_timeout.as_secs()
        );

        Self {
            reqwest,
            api_token,
            api_url,
            api_base_url,
            poll_interval,
            poll_timeout,
            queue_timeout,
            semaphore: Arc::new(Semaphore::new(max_concurrency)),
            inflight_predictions: Arc::new(AtomicUsize::new(0)),
        }
    }
}

impl ImageGenerator for ReplicateImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<'_, Result<CreatedImage, Error>> {
        Box::pin(async move {
            let request_started = Instant::now();
            let params = json!({
                "input": { "prompt": prompt }
            });

            let permit_wait_started = Instant::now();
            let permit = timeout(self.queue_timeout, self.semaphore.clone().acquire_owned())
                .await
                .map_err(|_| {
                    Error::ImageGeneration(format!(
                        "Timed out waiting for image generation slot after {}s",
                        self.queue_timeout.as_secs()
                    ))
                })?
                .map_err(|e| {
                    Error::ImageGeneration(format!("Failed to acquire concurrency permit: {}", e))
                })?;
            let permit_wait_ms = permit_wait_started.elapsed().as_millis();

            let _permit = permit;
            let _inflight_guard = InflightGuard::new(self.inflight_predictions.clone());
            let in_flight_now = self.inflight_predictions.load(Ordering::SeqCst);
            event!(
                Level::INFO,
                in_flight_predictions = in_flight_now,
                permit_wait_ms,
                prompt_len = params["input"]["prompt"]
                    .as_str()
                    .map(|s| s.len())
                    .unwrap_or(0),
                "Starting Replicate prediction request"
            );

            let create_resp = self
                .reqwest
                .post(&self.api_url)
                .header("Authorization", format!("Bearer {}", &self.api_token))
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .json(&params)
                .send()
                .await
                .map_err(|e| {
                    Error::ImageGeneration(format!("Failed to create Replicate prediction: {}", e))
                })?;
            let create_status = create_resp.status();
            if !create_status.is_success() {
                let body = create_resp.text().await.unwrap_or_default();
                event!(
                    Level::ERROR,
                    status = %create_status,
                    body = %body,
                    "Replicate create prediction failed"
                );
                return Err(Error::ImageGeneration(format!(
                    "Replicate create prediction failed with status {}: {}",
                    create_status, body
                )));
            }

            let mut prediction_json = create_resp.json::<Value>().await.map_err(|e| {
                Error::ImageGeneration(format!(
                    "Failed to parse Replicate prediction response JSON: {}",
                    e
                ))
            })?;
            let prediction_id = prediction_json
                .get("id")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let poll_url = prediction_json
                .get("urls")
                .and_then(|v| v.get("get"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| {
                    prediction_id.as_ref().map(|id| {
                        format!(
                            "{}/predictions/{}",
                            self.api_base_url.trim_end_matches('/'),
                            id
                        )
                    })
                });

            let poll_started = Instant::now();
            let mut last_status = String::new();
            loop {
                let status = prediction_json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let status_normalized = normalize_status(status);
                if status_normalized != last_status {
                    event!(
                        Level::INFO,
                        prediction_id = %prediction_id.as_deref().unwrap_or("unknown"),
                        status = %status,
                        "Replicate prediction status update"
                    );
                    last_status = status_normalized.clone();
                }

                if is_completed_status(&status_normalized) {
                    break;
                }
                if is_failed_status(&status_normalized) {
                    let err_msg = prediction_json
                        .get("error")
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "unknown error".to_string());
                    return Err(Error::ImageGeneration(format!(
                        "Replicate prediction {} failed with status '{}': {}",
                        prediction_id.as_deref().unwrap_or("unknown"),
                        status,
                        err_msg
                    )));
                }

                if poll_started.elapsed() > self.poll_timeout {
                    return Err(Error::ImageGeneration(format!(
                        "Replicate prediction {} timed out after {}s in status '{}'",
                        prediction_id.as_deref().unwrap_or("unknown"),
                        self.poll_timeout.as_secs(),
                        status
                    )));
                }

                let poll_url = poll_url.as_ref().ok_or_else(|| {
                    Error::ImageGeneration(
                        "Replicate prediction response missing both id and urls.get for polling"
                            .to_string(),
                    )
                })?;

                sleep(self.poll_interval).await;
                let poll_resp = self
                    .reqwest
                    .get(poll_url)
                    .header("Authorization", format!("Bearer {}", &self.api_token))
                    .header("Accept", "application/json")
                    .send()
                    .await
                    .map_err(|e| {
                        Error::ImageGeneration(format!(
                            "Failed to poll Replicate prediction {}: {}",
                            prediction_id.as_deref().unwrap_or("unknown"),
                            e
                        ))
                    })?;

                let poll_status = poll_resp.status();
                if !poll_status.is_success() {
                    let body = poll_resp.text().await.unwrap_or_default();
                    return Err(Error::ImageGeneration(format!(
                        "Polling Replicate prediction {} failed with status {}: {}",
                        prediction_id.as_deref().unwrap_or("unknown"),
                        poll_status,
                        body
                    )));
                }

                prediction_json = poll_resp.json::<Value>().await.map_err(|e| {
                    Error::ImageGeneration(format!(
                        "Failed to parse poll JSON for Replicate prediction {}: {}",
                        prediction_id.as_deref().unwrap_or("unknown"),
                        e
                    ))
                })?;
            }

            let url = extract_output_url(prediction_json.get("output")).ok_or_else(|| {
                Error::ImageGeneration(format!(
                    "Replicate prediction {} completed but has no usable output URL",
                    prediction_id.as_deref().unwrap_or("unknown")
                ))
            })?;

            let img_resp = self.reqwest.get(url).send().await.map_err(|e| {
                Error::ImageGeneration(format!(
                    "Failed to download image from Replicate output URL: {}",
                    e
                ))
            })?;
            let img_status = img_resp.status();
            if !img_status.is_success() {
                return Err(Error::ImageGeneration(format!(
                    "Failed to download image, status: {}",
                    img_status
                )));
            }
            let img_bytes = img_resp.bytes().await.map_err(|e| {
                Error::ImageGeneration(format!(
                    "Failed to read image bytes from Replicate output URL: {}",
                    e
                ))
            })?;
            event!(
                Level::INFO,
                prediction_id = %prediction_id.as_deref().unwrap_or("unknown"),
                elapsed_ms = request_started.elapsed().as_millis(),
                poll_elapsed_ms = poll_started.elapsed().as_millis(),
                in_flight_predictions = self.inflight_predictions.load(Ordering::SeqCst),
                "Replicate image generation completed"
            );
            Ok(CreatedImage {
                data: img_bytes.to_vec(),
                parameters: params.to_string(),
            })
        })
    }
}
