use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("Upstream error: {0}")]
    Upstream(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tile not found")]
    NotFound,

    #[error("Invalid tile coordinates")]
    InvalidCoordinates,

    #[error("Upstream returned {0}")]
    UpstreamStatus(u16),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::InvalidCoordinates => StatusCode::BAD_REQUEST,
            AppError::UpstreamStatus(code) => {
                StatusCode::from_u16(*code).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            AppError::Upstream(_) | AppError::Io(_) => StatusCode::BAD_GATEWAY,
        };

        tracing::error!(error = %self, "Request failed");
        (status, self.to_string()).into_response()
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
