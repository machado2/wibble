use futures::future::BoxFuture;
use std::fmt::Debug;

use crate::app_state::AppState;
use tokio::task::JoinHandle;
use tracing::{event, Level};

use crate::error::Error;

pub mod ai_horde;
pub mod fallback;
pub mod retrying;
pub mod stability;
pub mod huggingface;
pub mod replicate;

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

struct ImageGeneration {
    img: ImageToCreate,
    handle: JoinHandle<Result<CreatedImage, Error>>,
}

impl ImageGeneration {
    async fn wait(self) -> Result<ImageGenerated, Error> {
        let id = self.img.id.clone();
        let img = self.img;
        let data = match self.handle.await {
            Ok(Ok(g)) => g,
            Ok(Err(e)) => {
                event!(Level::ERROR, "Failed to create image: {}", e);
                return Err(e);
            }
            Err(e) => {
                event!(Level::ERROR, "Failed to join image creation: {}", e);
                return Err(Error::ImageGeneration(
                    "Failed to join image creation".into(),
                ));
            }
        };
        Ok(ImageGenerated {
            id,
            img,
            data: data.data,
            parameters: data.parameters,
        })
    }

    fn create(state: &AppState, img: ImageToCreate) -> ImageGeneration {
        let img_copy = img.clone();

        async fn create_image_inner(
            state: AppState,
            prompt: String,
        ) -> Result<CreatedImage, Error> {
            event!(Level::DEBUG, "Creating image for {}", &prompt);
            let img = state.image_generator.create_image(prompt.clone()).await;
            if img.is_err() {
                event!(Level::ERROR, "Failed to create image for {}", &prompt);
            } else {
                event!(Level::DEBUG, "Created image for {}", &prompt);
            }
            img
        }

        let fut = create_image_inner(state.clone(), img.prompt.clone());
        let handle = tokio::spawn(fut);
        let img = img_copy;
        ImageGeneration { img, handle }
    }
}

pub async fn generate_images(
    state: &AppState,
    images: Vec<ImageToCreate>,
) -> Result<Vec<ImageGenerated>, Error> {
    let images: Vec<_> = images
        .iter()
        .map(|img| ImageGeneration::create(state, img.clone()))
        .collect();
    let mut images_generated = Vec::new();
    for img in images {
        let img = img.wait().await;
        if let Ok(img) = img {
            images_generated.push(img);
        }
    }
    Ok(images_generated)
}
