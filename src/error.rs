use axum::response::{IntoResponse, Response};
use sea_orm::DbErr;
use static_assertions::assert_impl_all;
use tracing::{event, Level};

#[derive(thiserror::Error, Debug)]
pub enum Error
where
    Self: Send + Sync,
{
    #[error("Not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error("LLM error: {0}")]
    Llm(String),
    #[error("Image generation error: {0}")]
    ImageGeneration(String),
    #[error("Censored by Image generator")]
    ImageCensored,
    #[error("Rate limited")]
    RateLimited,
    #[error("Image error: {0}")]
    Image(#[from] image::ImageError),
    #[error("Template error: {0}")]
    Template(tera::Error),
}

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
