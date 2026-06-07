use serde::Serialize;
use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorPayload {
    pub code: &'static str,
    pub message: String,
    pub recoverable: bool,
}

impl AppError {
    pub fn payload(&self) -> ErrorPayload {
        let (code, recoverable) = match self {
            Self::Validation(_) => ("validation_failed", true),
            Self::Conflict(_) => ("conflict", true),
            Self::NotFound(_) => ("not_found", true),
            Self::Io(_) => ("io_failed", true),
            Self::Json(_) => ("invalid_json", true),
        };
        ErrorPayload {
            code,
            message: self.to_string(),
            recoverable,
        }
    }
}
