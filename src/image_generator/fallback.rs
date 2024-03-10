use futures::future::BoxFuture;
use tracing::{event, Level};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Clone, Debug)]
pub struct FallbackImageGenerator<G1, G2>
where
    G1: ImageGenerator,
    G2: ImageGenerator,
{
    generator1: G1,
    generator2: G2,
}

impl<G1, G2> FallbackImageGenerator<G1, G2>
where
    G1: ImageGenerator,
    G2: ImageGenerator,
{
    pub fn new(generator1: G1, generator2: G2) -> Self {
        Self {
            generator1,
            generator2,
        }
    }
}

impl<G1, G2> ImageGenerator for FallbackImageGenerator<G1, G2>
where
    G1: ImageGenerator,
    G2: ImageGenerator,
{
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            let r = self.generator1.create_image(prompt.clone()).await;
            match r {
                Ok(_) => r,
                Err(e) => {
                    event!(
                        Level::ERROR,
                        "Error generating image with generator 1: {:?}",
                        e
                    );
                    self.generator2.create_image(prompt).await
                }
            }
        })
    }
}
