use axum::response::{IntoResponse, Response};
use sea_orm::DbErr;
use static_assertions::assert_impl_all;
use tracing::{event, Level};
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NotFound,
    Database(String),
    Llm(String),
    ImageGeneration(String),
    ImageCensored,
    RateLimited,
    Image(image::ImageError),
    Template(tera::Error),
    Storage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::NotFound => write!(f, "Not found"),
            Error::Database(msg) => write!(f, "Database error: {}", msg),
            Error::Llm(msg) => write!(f, "LLM error: {}", msg),
            Error::ImageGeneration(msg) => write!(f, "Image generation error: {}", msg),
            Error::ImageCensored => write!(f, "Censored by Image generator"),
            Error::RateLimited => write!(f, "Rate limited"),
            Error::Image(err) => write!(f, "Image error: {}", err),
            Error::Template(err) => write!(f, "Template error: {}", err),
            Error::Storage(msg) => write!(f, "Storage error: {}", msg),
        }
    }
}

impl std::error::Error for Error {}

assert_impl_all!(Error: Send, Sync);

pub type Result<T> = std::result::Result<T, Error>;

impl From<DbErr> for Error {
    fn from(e: DbErr) -> Self {
        Error::Database(e.to_string())
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        event!(Level::ERROR, "{}", self);
        let status = match self {
            Error::NotFound => http::StatusCode::NOT_FOUND,
            Error::RateLimited => axum::http::StatusCode::TOO_MANY_REQUESTS,
            _ => axum::http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, self.to_string()).into_response()
    }
}
