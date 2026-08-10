// curl -f -sS "https://api.stability.ai/v2beta/stable-image/generate/sd3" \
//   -H "authorization: Bearer sk-MYAPIKEY" \
//   -H "accept: image/*" \
//   -F prompt="Lighthouse on a cliff overlooking the ocean" \
//   -F output_format="jpeg" \
//   -o "./lighthouse.jpeg"

use std::env;

use futures::future::BoxFuture;
use tracing::{event, Level};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
pub struct StabilityImageGenerator {
    reqwest: reqwest::Client,
    api_key: String,
}

impl StabilityImageGenerator {
    pub(crate) fn new() -> Self {
        let api_key = env::var("STABILITY_AI_API_KEY").expect("STABILITY_AI_API_KEY must be set");
        Self {
            reqwest: reqwest::Client::new(),
            api_key,
        }
    }
}

impl ImageGenerator for StabilityImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<'_, Result<CreatedImage, Error>> {
        Box::pin(async move {
            // "content-type: must be multipart/form-data
            let resp = self
                .reqwest
                .post("https://api.stability.ai/v2beta/stable-image/generate/sd3")
                .header("authorization", format!("Bearer {}", &self.api_key))
                .header("accept", "image/*")
                .multipart(
                    reqwest::multipart::Form::new()
                        .text("prompt", prompt)
                        .text("output_format", "jpeg"),
                )
                .send()
                .await
                .map_err(|e| {
                    Error::ImageGeneration(format!(
                        "Failed to send request for image creation on Stability AI Api: {}",
                        e
                    ))
                })?;
            let status_code = resp.status();
            if !status_code.is_success() {
                event!(Level::ERROR, "Stability AI response: {:?}", resp);
                let body = resp.text().await.map_err(|e| {
                    Error::ImageGeneration(format!(
                        "Failed to read response body from Stability AI Api: {}",
                        e
                    ))
                })?;
                event!(Level::ERROR, "Stability AI response body: {:?}", body);
                return Err(Error::ImageGeneration(format!(
                    "Failed to generate image: {}",
                    status_code
                )));
            }
            let resp = resp.bytes().await.map_err(|e| {
                Error::ImageGeneration(format!(
                    "Failed to read response from Stability AI Api: {}",
                    e
                ))
            })?;
            Ok(CreatedImage {
                data: resp.to_vec(),
                parameters: "Stable Diffusion 3".to_string(),
            })
        })
    }
}
