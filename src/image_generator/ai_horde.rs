#![allow(dead_code)]

use std::env;
use std::future::Future;
use std::io::Cursor;
use std::time::Duration;

use backoff::ExponentialBackoff;
use backoff::future::retry;
use futures::future::BoxFuture;
use http::StatusCode;
use image::ImageFormat;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{json, Value};
use tracing::{error, event, Level};

use crate::error::Error;
use crate::image_generator::{CreatedImage, ImageGenerator};

#[derive(Debug, Clone)]
struct GenerateImageResponse {
    id: String,
    url: Option<String>,
    model: String,
    parameters: String
}

#[derive(Debug, Clone)]
pub struct AiHordeImageGenerator {
    client: reqwest::Client,
    headers: HeaderMap,
    style: Option<Value>,
}

#[derive(thiserror::Error, Debug)]
pub enum HordeError
where
    Self: Send + Sync,
{
    #[error("AI Horde server error: {0}")]
    ServerError(String),
    #[error("Censored by Image generator")]
    ImageCensored,
    #[error("Rate limited")]
    RateLimited,
    #[error("Invalid image: {0}")]
    Image(#[from] image::ImageError),
    #[error("Unexpected error: {0}")]
    Unexpected(String),
    #[error("Image not ready")]
    Pending,
}

static BASE_URL: &str = "https://aihorde.net/api/v2";

static TIMEOUT: Duration = Duration::from_secs(120);

trait AndAwait<T, E> {
    async fn and_await(self) -> Result<T, E>;
}

impl<T, FT, E> AndAwait<T, E> for Result<FT, E>
where
    FT: Future<Output = Result<T, E>>,
{
    async fn and_await(self) -> Result<T, E> {
        match self {
            Ok(f) => f.await,
            Err(e) => Err(e),
        }
    }
}

impl AiHordeImageGenerator {
    pub fn new() -> AiHordeImageGenerator {
        let mut headers = HeaderMap::new();
        let api_key = env::var("AI_HORDE_API_KEY").expect("AI_HORDE_API_KEY not set");
        headers.insert(
            "apikey",
            HeaderValue::from_str(&api_key)
                .expect("Invalid AI Horde API KEY")
                .to_owned(),
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        let style: Option<Value> = env::var("AI_HORDE_STYLE")
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok());
        AiHordeImageGenerator {
            client: reqwest::Client::new(),
            style,
            headers,
        }
    }

    fn check_status_code(status: StatusCode) -> Result<(), HordeError> {
        if status.is_success() {
            return Ok(());
        }
        let code = status.as_u16();
        match code {
            429 => Err(HordeError::RateLimited),
            500..=599 => Err(HordeError::ServerError(format!(
                "AI Horde status code: {}",
                code
            ))),
            _ => Err(HordeError::Unexpected(format!(
                "AI horde status code: {}",
                code
            ))),
        }
    }

    async fn post(&self, path: &str, body: Value) -> Result<Value, HordeError> {
        event!(
            Level::DEBUG,
            "POST to AI Horde: {} with body: {:?}",
            path,
            body
        );
        let res = self
            .client
            .post(format!("{}{}", BASE_URL, path))
            .headers(self.headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                HordeError::Unexpected(format!("Error sending request to AI horde: {:?}", e))
            })?;
        let status = res.status();
        Self::check_status_code(status)?;
        let r = res.json::<Value>().await.map_err(|e| {
            HordeError::Unexpected(format!("Error parsing AI horde response: {:?}", e))
        })?;
        Ok(r)
    }

    async fn get(&self, path: &str) -> Result<Value, HordeError> {
        let r = self
            .client
            .get(format!("{}{}", BASE_URL, path))
            .send()
            .await
            .map_err(|e| {
                HordeError::Unexpected(format!("Error on GET {} AI horde: {:?}", path, e))
            })?;
        let status_code = r.status();
        Self::check_status_code(status_code)?;
        let r = r.json::<Value>().await.map_err(|e| {
            HordeError::Unexpected(format!("Error parsing AI horde response: {:?}", e))
        })?;
        Ok(r)
    }

    async fn ai_horde_generate(&self, prompt: &str) -> Result<GenerateImageResponse, HordeError> {
        let (final_prompt, parameters, model) = match &self.style {
            Some(style) => {
                let final_prompt = match style.get("prompt") {
                    Some(style_prompt) => style_prompt
                        .as_str()
                        .unwrap()
                        .replace("{p}", prompt)
                        .replace("{np}", "")
                        .to_string(),
                    None => prompt.to_string(),
                };
                let mut parameters = style.clone().as_object_mut().unwrap().clone();
                let model: Option<String> = parameters
                    .get("model")
                    .and_then(|m| m.as_str())
                    .map(|m| m.to_string());
                parameters.remove("prompt");
                if model.is_some() {
                    parameters.remove("model");
                }
                if !parameters.contains_key("sampler_name") {
                    parameters.insert("sampler_name".to_string(), json!("k_dpmpp_sde"));
                    if !parameters.contains_key("karras") {
                        parameters.insert("karras".to_string(), json!(true));
                    }
                }
                let parameters = serde_json::to_value(parameters).ok().unwrap();
                (final_prompt, parameters, model)
            }
            None => (prompt.to_string(), json!({}), None),
        };

        let mut body = json!({
            "prompt": final_prompt,
            "params": parameters,
            "nsfw": true,
            "censor_nsfw": false,
            "slow_workers": false,
        });
        if let Some(model) = model.clone() {
            body["models"] = json!([model]);
        }

        let res = self.post("/generate/async", body.clone()).await?;
        let id = res
            .get("id")
            .and_then(|id| id.as_str())
            .ok_or(HordeError::Unexpected(
                "AI horde response without id".into(),
            ))?
            .to_string();
        
        let parameters_str = serde_json::to_string(&body).unwrap();
        let generated_image_response = GenerateImageResponse {
            id,
            url: None,
            model: model.unwrap_or(String::from("unspecified")).to_string(),
            parameters: parameters_str
        };
        Ok(generated_image_response)
    }

    async fn get_status(&self, id: &str) -> Result<String, HordeError> {
        let j = self.get(&format!("/generate/status/{}", id)).await?;
        if j["generations"][0]["censored"].as_bool() == Some(true) {
            return Err(HordeError::ImageCensored);
        }
        if j["faulted"].as_bool() == Some(true) {
            return Err(HordeError::ServerError("AI horde faulted".into()));
        }

        if j["done"].as_bool() == Some(true) {
            let gen = &j["generations"][0];
            Ok(gen["img"]
                .as_str()
                .ok_or(HordeError::ServerError(
                    "Missing img from AI horde success message".into(),
                ))?
                .to_string())
        } else {
            Err(HordeError::Pending)
        }
    }

    async fn generate_with_backoff(&self, prompt: String) -> Result<GenerateImageResponse, HordeError> {
        retry(
            ExponentialBackoff {
                max_elapsed_time: Some(TIMEOUT),
                ..ExponentialBackoff::default()
            },
            || async { Ok(self.ai_horde_generate(&prompt).await?) },
        )
        .await
    }

    fn transient_error(e: HordeError) -> backoff::Error<HordeError> {
        backoff::Error::Transient {
            err: e,
            retry_after: None,
        }
    }

    async fn generate_image(&self, prompt: &str) -> Result<GenerateImageResponse, HordeError> {
        let gen_response = self.generate_with_backoff(prompt.to_string()).await?;
        let id = gen_response.id;
        retry(
            ExponentialBackoff {
                max_elapsed_time: Some(TIMEOUT),
                ..ExponentialBackoff::default()
            },
            || async {
                let status = self.get_status(&id).await;
                let r: Result<String, backoff::Error<HordeError>> = match status {
                    Ok(s) => Ok(s),
                    Err(e) => match e {
                        HordeError::ServerError(_)
                        | HordeError::RateLimited
                        | HordeError::Pending => Err(backoff::Error::Transient {
                            err: e,
                            retry_after: None,
                        }),
                        _ => Err(backoff::Error::Permanent(e)),
                    },
                };
                r.map(|s| GenerateImageResponse {
                    id: id.clone(),
                    url: Some(s),
                    model: gen_response.model.clone(),
                    parameters: gen_response.parameters.clone()
                })
            },
        )
        .await
    }

    async fn and_await<T, FT, E>(o: Result<FT, E>) -> Result<T, E>
    where
        FT: Future<Output = Result<T, E>>,
    {
        match o {
            Ok(f) => f.await,
            Err(e) => Err(e),
        }
    }

    async fn create_image(&self, prompt: String) -> Result<CreatedImage, HordeError> {
        let generate_response = self.generate_image(&prompt).await?;
        let response = retry(ExponentialBackoff::default(), || async {
            Ok(reqwest::get(generate_response.url.clone().unwrap()).await?)
        })
        .await
        .map_err(|e| {
            HordeError::ServerError(format!("Error fetching image from AI Horde: {:?}", e))
        })?;
        let bytes = response.bytes().await.map_err(|e| {
            HordeError::ServerError(format!("Error fetching image from AI Horde: {:?}", e))
        })?;
        let img = image::load_from_memory(&bytes).map_err(|e| {
            HordeError::ServerError(format!("Invalid image from AI Horde: {:?}", e))
        })?;
        let mut buffer = Cursor::new(Vec::new());
        img.write_to(&mut buffer, ImageFormat::Jpeg)
            .map_err(|e| HordeError::Unexpected(format!("Error encoding image to Jpeg: {:?}", e)))
            .map_err(|_| {
                HordeError::Unexpected("Error writing jpeg to memory stream".to_string())
            })?;
        Ok(CreatedImage {
            data: buffer.into_inner(),
            parameters: generate_response.parameters,
        })
    }
}

impl ImageGenerator for AiHordeImageGenerator {
    fn create_image(&self, prompt: String) -> BoxFuture<Result<CreatedImage, Error>> {
        Box::pin(async move {
            self.create_image(prompt).await.map_err(|he| match he {
                HordeError::ImageCensored => {
                    error!("Image censored");
                    Error::ImageCensored
                }
                HordeError::RateLimited => {
                    error!("Rate limited");
                    Error::RateLimited
                }
                _ => {
                    error!("{}", he.to_string());
                    Error::ImageGeneration(he.to_string())
                }
            })
        })
    }
}
