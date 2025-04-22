use std::env;

use futures::future::BoxFuture;
use tracing::{event, Level};
use serde_json::json;

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
            .unwrap_or_else(|_| "https://api-inference.huggingface.co/models/stabilityai/stable-diffusion-xl-base-1.0".to_string()); // Default SDXL model
        Self {
            reqwest: reqwest::Client::new(),
            api_key,
            api_url,
        }
    }
}

impl ImageGenerator for HuggingFaceImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            let api_url = &self.api_url; // Use configured or default API URL
            let params = json!({
                "inputs": prompt,
                "num_inference_steps": 50,
                "guidance_scale": 5.0,
                "negative_prompt": "bad art, low quality, blurry, out of focus, simplistic colors, boring drawings"
            });
            let resp = self
                .reqwest
                .post(api_url) // Use the API URL here
                .header("Authorization", format!("Bearer {}", &self.api_key)) // API Key for HuggingFace
                .header("Content-Type", "application/json") // HuggingFace API expects JSON
                .header("Accept", "image/*") // Expect image response
                .body(params.to_string())
                .send()
                .await
                .map_err(|e| {
                    Error::ImageGeneration(format!(
                        "Failed to send request for image creation on Hugging Face Inference API: {}",
                        e
                    ))
                })?;

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

            let content_type = resp.headers().get("content-type").and_then(|value| value.to_str().ok()).and_then(|s| Some(s.to_string()));
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