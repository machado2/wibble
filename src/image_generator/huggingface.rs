use std::env;

use futures::future::BoxFuture;
use tracing::{event, Level};
use serde_json::json;
use tokio::time::{sleep, Duration};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
pub struct HuggingFaceImageGenerator {
    reqwest: reqwest::Client,
    api_key: String,
    api_url: String, // Added API URL to be configurable
}

impl HuggingFaceImageGenerator {
    pub(crate) fn new() -> Self {
        let api_key = env::var("HUGGINGFACE_API_KEY").expect("HUGGINGFACE_API_KEY must be set");
        // You might want to make the model configurable via environment variable as well
        let api_url = env::var("HUGGINGFACE_API_URL")
            .unwrap_or_else(|_| "https://router.huggingface.co/hf-inference/models/black-forest-labs/FLUX.1-schnell".to_string()); // Default Flux.1-schnell model
        Self {
            reqwest: reqwest::Client::new(),
            api_key,
            api_url,
        }
    }
}

impl ImageGenerator for HuggingFaceImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<'_, Result<CreatedImage, Error>> {
        Box::pin(async move {
            let api_url = &self.api_url; // Use configured or default API URL
            let params = json!({
                "inputs": prompt
            });
            let mut retries = 0;
            let max_retries = 3;
            let resp = loop {
                let attempt = retries + 1;
                match self
                    .reqwest
                    .post(api_url)
                    .header("Authorization", format!("Bearer {}", &self.api_key))
                    .header("Content-Type", "application/json")
                    .header("Accept", "image/*")
                    .header("X-Use-Queue", "true")
                    .body(params.to_string())
                    .send()
                    .await
                {
                    Ok(response) => {
                        if !response.status().is_server_error() || attempt >= max_retries {
                            break response;
                        }
                        event!(Level::WARN, "Server error {}. Retrying attempt {}/{}...", response.status(), attempt, max_retries);
                    }
                    Err(err) => {
                        if attempt >= max_retries {
                            return Err(Error::ImageGeneration(format!(
                                "Failed to send request for image creation on Hugging Face Inference API after {} attempts: {}",
                                attempt, err
                            )));
                        }
                        event!(Level::WARN, "Request error: {}. Retrying attempt {}/{}...", err, attempt, max_retries);
                    }
                }
                retries += 1;
                sleep(Duration::from_secs(2u64.pow(retries))).await;
            };

            let status_code = resp.status();
            if !status_code.is_success() {
                event!(Level::ERROR, "Hugging Face API response: {:?}", resp);
                let body = resp.text().await.map_err(|e| { // Use await here
                    Error::ImageGeneration(format!(
                        "Failed to read response body from Hugging Face Inference API: {}",
                        e
                    ))
                })?;
                event!(Level::ERROR, "Hugging Face API response body: {:?}", body);
                return Err(Error::ImageGeneration(format!(
                    "Failed to generate image: {}",
                    status_code
                )));
            }

            let content_type = resp.headers().get("content-type").and_then(|value| value.to_str().ok()).map(|s| s.to_string());
            if let Some(content_type) = content_type {
                if !content_type.starts_with("image/") {
                    let body = resp.text().await.map_err(|e| {
                        Error::ImageGeneration(format!(
                            "Failed to read response body from Hugging Face Inference API: {}",
                            e
                        ))
                    })?;
                    event!(Level::ERROR, "Hugging Face API response body (not image): {:?}", body);
                    return Err(Error::ImageGeneration(format!(
                        "Expected image content, but received: Content-Type: {}, Body: {}",
                        content_type, body
                    )));
                }
            } else {
                event!(Level::WARN, "Content-Type header missing from Hugging Face API response.");
            }


            let resp_bytes = resp.bytes().await.map_err(|e| { // Use await here
                Error::ImageGeneration(format!(
                    "Failed to read response from Hugging Face Inference API: {}",
                    e
                ))
            })?;

            Ok(CreatedImage {
                data: resp_bytes.to_vec(),
                parameters: params.to_string(),
            })
        })
    }
}