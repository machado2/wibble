use std::env;

use futures::future::BoxFuture;
use tracing::{event, Level};
use serde_json::{json, Value};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
pub struct ReplicateImageGenerator {
    reqwest: reqwest::Client,
    api_token: String,
    api_url: String,
}

impl ReplicateImageGenerator {
    pub(crate) fn new() -> Self {
        let api_token = env::var("REPLICATE_API_TOKEN").expect("REPLICATE_API_TOKEN must be set");
        let api_url = env::var("REPLICATE_API_URL")
            .unwrap_or_else(|_| "https://api.replicate.com/v1/models/black-forest-labs/flux-schnell/predictions".to_string());
        Self {
            reqwest: reqwest::Client::new(),
            api_token,
            api_url,
        }
    }
}

impl ImageGenerator for ReplicateImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            let api_url = &self.api_url;
            let params = json!({
                "input": { "prompt": prompt }
            });
            let resp = self.reqwest
                .post(api_url)
                .header("Authorization", format!("Bearer {}", &self.api_token))
                .header("Content-Type", "application/json")
                .header("Prefer", "wait")
                .body(params.to_string())
                .send()
                .await
                .map_err(|e| Error::ImageGeneration(format!("Failed to send request to Replicate API: {}", e)))?;
            let status = resp.status();
            if !status.is_success() {
                event!(Level::ERROR, "Replicate API response: {:?}", resp);
                let body = resp.text().await.map_err(|e| Error::ImageGeneration(format!("Failed to read response body from Replicate API: {}", e)))?;
                event!(Level::ERROR, "Replicate API response body: {:?}", body);
                return Err(Error::ImageGeneration(format!("Failed to generate image: {}", status)));
            }
            let result_json = resp.json::<Value>().await.map_err(|e| Error::ImageGeneration(format!("Failed to parse JSON from Replicate API: {}", e)))?;
            let outputs = result_json.get("output")
                .and_then(|o| o.as_array())
                .ok_or_else(|| Error::ImageGeneration("Invalid output from Replicate API".into()))?;
            if outputs.is_empty() {
                return Err(Error::ImageGeneration("No outputs in Replicate API response".into()));
            }
            let url = outputs[0].as_str().ok_or_else(|| Error::ImageGeneration("Invalid output URL from Replicate API".into()))?;
            let img_resp = self.reqwest
                .get(url)
                .send()
                .await
                .map_err(|e| Error::ImageGeneration(format!("Failed to download image from Replicate output URL: {}", e)))?;
            let img_status = img_resp.status();
            if !img_status.is_success() {
                return Err(Error::ImageGeneration(format!("Failed to download image, status: {}", img_status)));
            }
            let img_bytes = img_resp.bytes().await.map_err(|e| Error::ImageGeneration(format!("Failed to read image bytes from Replicate output URL: {}", e)))?;
            Ok(CreatedImage {
                data: img_bytes.to_vec(),
                parameters: params.to_string(),
            })
        })
    }
} 