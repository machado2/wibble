use async_openai::config::OpenAIConfig;
use std::io::Cursor;

use async_openai::types::{
    CreateImageRequestArgs, Image, ImageModel, ImageQuality, ImageSize, ResponseFormat,
};
use futures::future::BoxFuture;
use image::ImageFormat;

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
pub struct DallEImageGenerator {
    openai: async_openai::Client<OpenAIConfig>,
    reqwest: reqwest::Client,
}

impl DallEImageGenerator {
    pub fn new() -> Self {
        Self {
            openai: async_openai::Client::new(),
            reqwest: reqwest::Client::new(),
        }
    }
}

impl ImageGenerator for DallEImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            let openai = &self.openai;
            let model = ImageModel::DallE3;
            let model_name = model.to_string();
            let req = CreateImageRequestArgs::default()
                .prompt(prompt)
                .n(1)
                .response_format(ResponseFormat::Url)
                .model(model)
                .size(ImageSize::S1024x1024)
                .quality(ImageQuality::Standard)
                .build()
                .map_err(|e| {
                    Error::ImageGeneration(format!("Failed to build request for Dall-e: {}", e))
                })?;
            let res = openai.images().create(req).await.map_err(|e| {
                Error::ImageGeneration(format!("Failed to create image on Dall-e: {}", e))
            })?;
            let data = res.data.first().ok_or(Error::NotFound)?;
            match data.as_ref() {
                Image::Url { url, .. } => {
                    let image_bytes = self
                        .reqwest
                        .get(url)
                        .send()
                        .await
                        .map_err(|e| {
                            Error::ImageGeneration(format!("Failed to fetch image: {}", e))
                        })?
                        .bytes()
                        .await
                        .map_err(|e| {
                            Error::ImageGeneration(format!("Failed to fetch image: {}", e))
                        })?;
                    let img = image::load_from_memory(&image_bytes)?;
                    let mut buffer = Cursor::new(Vec::new());
                    img.write_to(&mut buffer, ImageFormat::Jpeg).map_err(|e| {
                        Error::ImageGeneration(format!("Failed to encode as jpeg: {}", e))
                    })?;
                    Ok(CreatedImage {
                        data: buffer.into_inner(),
                        model: model_name,
                    })
                }
                _ => Err(Error::NotFound),
            }
        })
    }
}
