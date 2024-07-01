use thiserror::Error;
use worker::Error as WorkerError;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Worker error: {0}")]
    Worker(#[from] WorkerError),
    #[error("Internal error: {0}")]
    Internal(String),
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),
    #[error("File too large: {0}")]
    FileTooLarge(String),
}

impl From<AppError> for WorkerError {
    fn from(error: AppError) -> Self {
        match error {
            AppError::Worker(e) => e,
            AppError::Internal(msg) => WorkerError::RustError(msg),
            AppError::NotFound(msg) => WorkerError::RustError(msg),
            AppError::BadRequest(msg) => WorkerError::RustError(msg),
            AppError::Unauthorized(msg) => WorkerError::RustError(msg),
            AppError::RateLimitExceeded(msg) => WorkerError::RustError(msg),
            AppError::FileTooLarge(msg) => WorkerError::RustError(msg),
        }
    }
}