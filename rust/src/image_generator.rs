use futures::future::BoxFuture;
use futures::stream::{self, StreamExt};
use std::env;
use std::fmt::Debug;
use std::time::Instant;

use crate::app_state::AppState;
use tracing::{event, Level};

use crate::error::Error;

pub mod ai_horde;
pub mod fallback;
pub mod huggingface;
pub mod replicate;
pub mod stability;

pub struct CreatedImage {
    pub data: Vec<u8>,
    pub parameters: String,
}

pub trait ImageGenerator: Debug + Send + Sync {
    fn create_image(&self, prompt: String) -> BoxFuture<'_, Result<CreatedImage, Error>>;
}

#[derive(Clone)]
pub struct ImageToCreate {
    pub id: String,
    pub caption: String,
    pub prompt: String,
}

pub struct ImageGenerated {
    pub id: String,
    pub img: ImageToCreate,
    pub data: Vec<u8>,
    pub parameters: String,
}

pub async fn generate_images(
    state: &AppState,
    images: Vec<ImageToCreate>,
) -> Result<Vec<ImageGenerated>, Error> {
    let max_parallel = env::var("IMAGE_MAX_PARALLEL_PER_ARTICLE")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(2);
    event!(
        Level::INFO,
        total_images = images.len(),
        max_parallel,
        "Starting article image batch generation"
    );

    let stream = stream::iter(images.into_iter().map(|img| {
        let state = state.clone();
        async move {
            let started = Instant::now();
            let image_id = img.id.clone();
            event!(Level::DEBUG, image_id = %image_id, "Creating image");
            match state.image_generator.create_image(img.prompt.clone()).await {
                Ok(data) => {
                    event!(
                        Level::INFO,
                        image_id = %image_id,
                        elapsed_ms = started.elapsed().as_millis(),
                        "Created image"
                    );
                    Some(ImageGenerated {
                        id: image_id,
                        img,
                        data: data.data,
                        parameters: data.parameters,
                    })
                }
                Err(e) => {
                    event!(
                        Level::ERROR,
                        image_id = %image_id,
                        elapsed_ms = started.elapsed().as_millis(),
                        error = %e,
                        "Failed to create image"
                    );
                    None
                }
            }
        }
    }))
    .buffered(max_parallel);

    let mut images_generated = Vec::new();
    futures::pin_mut!(stream);
    while let Some(img) = stream.next().await {
        if let Some(img) = img {
            images_generated.push(img);
        }
    }
    event!(
        Level::INFO,
        generated_images = images_generated.len(),
        "Finished article image batch generation"
    );
    Ok(images_generated)
}
