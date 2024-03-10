use backoff::future::retry;
use futures::future::BoxFuture;

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Clone, Debug)]
pub struct RetryingImageGenerator<G>
where
    G: ImageGenerator,
{
    generator: G,
}

impl<G> RetryingImageGenerator<G>
where
    G: ImageGenerator,
{
    pub fn new(generator: G) -> Self {
        Self { generator }
    }
}

impl<G> ImageGenerator for RetryingImageGenerator<G>
where
    G: ImageGenerator,
{
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            let l = || async {
                let r: Result<CreatedImage, backoff::Error<Error>> = self
                    .generator
                    .create_image(prompt.clone())
                    .await
                    .map_err(|e| backoff::Error::Transient {
                        err: e,
                        retry_after: None,
                    });
                r
            };
            let r = retry(
                backoff::ExponentialBackoff {
                    max_elapsed_time: Some(std::time::Duration::from_secs(60 * 60)),
                    ..backoff::ExponentialBackoff::default()
                },
                l,
            )
            .await;
            r.map_err(|e| Error::ImageGeneration(format!("Error generating image: {:?}", e)))
        })
    }
}
