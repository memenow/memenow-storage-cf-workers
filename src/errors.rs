//! # Error Handling and Response Management
//!
//! This module provides comprehensive error handling for the file storage service.
//! It defines structured error types, automatic HTTP response generation, and
//! conversion from various error sources throughout the application.
//!
//! ## Error Design Principles
//!
//! - **Structured Errors**: Each error type contains relevant context information
//! - **HTTP Mapping**: Errors automatically map to appropriate HTTP status codes
//! - **Client-Friendly**: Error messages are clear and actionable for API consumers
//! - **JSON Format**: All error responses use consistent JSON structure
//! - **Timestamp Tracking**: Errors include UTC timestamps for debugging
//!
//! ## Error Categories
//!
//! - **Client Errors (4xx)**: Missing fields, invalid input, file size limits
//! - **Server Errors (5xx)**: Storage failures, configuration issues, internal errors
//! - **Service Errors (502)**: External service failures (R2, KV)
//!
//! ## Example Error Response
//!
//! ```json
//! {
//!   "error": {
//!     "code": "FILE_TOO_LARGE",
//!     "message": "File size 11000000000 exceeds maximum allowed 10737418240",
//!     "timestamp": "2024-01-15T10:30:00Z"
//!   }
//! }
//! ```

use thiserror::Error;
use worker::{Error as WorkerError, Response, Result};
use serde_json::json;

/// Application error enumeration covering all possible error conditions.
///
/// This enum uses the `thiserror` crate to provide automatic `Error` trait
/// implementation and display formatting. Each variant includes relevant
/// context data to provide meaningful error messages to API consumers.
///
/// # Error Categories
///
/// - **Validation Errors**: Missing or invalid input data
/// - **Business Logic Errors**: Upload state violations, size limits
/// - **Storage Errors**: Failures in R2, KV, or Durable Object operations
/// - **System Errors**: Configuration issues, rate limiting, internal failures
#[derive(Error, Debug)]
pub enum AppError {
    /// Required field is missing from request payload or headers.
    #[error("Missing required field: {field}")]
    MissingField { 
        /// Name of the missing field
        field: String 
    },

    /// Request validation error (replaces MissingField and InvalidField for simplicity).
    #[error("Validation error: {message}")]
    ValidationError { 
        /// Validation error message
        message: String 
    },

    /// Resource not found error.
    #[error("Not found: {message}")]
    NotFoundError { 
        /// Not found error message
        message: String 
    },
    
    /// Field contains invalid or malformed data.
    #[error("Invalid field value: {field} - {reason}")]
    InvalidField { 
        /// Name of the invalid field
        field: String, 
        /// Explanation of why the field is invalid
        reason: String 
    },
    
    /// File size exceeds the configured maximum limit.
    #[error("File size {size} exceeds maximum allowed {max}")]
    FileSizeExceeded { 
        /// Actual file size in bytes
        size: u64, 
        /// Maximum allowed size in bytes
        max: u64 
    },
    
    /// Upload session not found in storage.
    #[error("Upload not found: {upload_id}")]
    UploadNotFound { 
        /// Upload identifier that was not found
        upload_id: String 
    },
    
    /// Attempt to modify an upload that has already been completed.
    #[error("Upload already completed: {upload_id}")]
    UploadAlreadyCompleted { 
        /// Upload identifier for the completed upload
        upload_id: String 
    },
    
    /// Attempt to operate on a cancelled upload.
    #[error("Upload cancelled: {upload_id}")]
    UploadCancelled { 
        /// Upload identifier for the cancelled upload
        upload_id: String 
    },
    
    /// Chunk index is invalid or out of sequence.
    #[error("Invalid chunk index: {index}")]
    InvalidChunkIndex { 
        /// The invalid chunk index
        index: u16 
    },
    
    /// R2 storage operation failure.
    #[error("R2 storage error: {message}")]
    R2Error { 
        /// Detailed error message from R2 operation
        message: String 
    },
    
    /// KV storage operation failure.
    #[error("KV storage error: {message}")]
    KvError { 
        /// Detailed error message from KV operation
        message: String 
    },
    
    /// D1 Database operation failure.
    #[error("Database error: {message}")]
    DatabaseError { 
        /// Detailed error message from database operation
        message: String,
    },
    
    /// Configuration loading or validation error.
    #[error("Configuration error: {message}")]
    ConfigError { 
        /// Detailed configuration error message
        message: String 
    },
    
    /// Authentication or authorization failure.
    #[error("Authentication error: {message}")]
    AuthError { 
        /// Detailed authentication error message
        message: String 
    },
    
    /// Rate limiting threshold exceeded.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    
    /// Unexpected internal server error.
    #[error("Internal server error: {message}")]
    InternalError { 
        /// Detailed internal error message
        message: String 
    },
}

impl AppError {
    /// Converts the application error into an HTTP response.
    ///
    /// This method maps each error variant to an appropriate HTTP status code
    /// and creates a structured JSON response with error details. The response
    /// includes a machine-readable error code, human-readable message, and
    /// timestamp for debugging purposes.
    ///
    /// # Returns
    ///
    /// Returns a `Result<Response>` containing the HTTP error response.
    ///
    /// # Error Response Format
    ///
    /// ```json
    /// {
    ///   "error": {
    ///     "code": "ERROR_CODE",
    ///     "message": "Human-readable error description",
    ///     "timestamp": "2024-01-15T10:30:00Z"
    ///   }
    /// }
    /// ```
    ///
    /// # Status Code Mapping
    ///
    /// - **400**: Client errors (missing/invalid fields, invalid chunk index)
    /// - **401**: Authentication errors
    /// - **404**: Resource not found (upload not found)
    /// - **409**: Conflict errors (upload already completed/cancelled)
    /// - **413**: Payload too large (file size exceeded)
    /// - **429**: Rate limit exceeded
    /// - **500**: Internal server errors (config, durable object)
    /// - **502**: External service errors (R2, KV)
    pub fn to_response(&self) -> Result<Response> {
        let (status, error_code, message) = match self {
            AppError::MissingField { field } => (
                400,
                "MISSING_FIELD",
                format!("Missing required field: {}", field),
            ),
            AppError::ValidationError { message } => (
                400,
                "VALIDATION_ERROR",
                message.clone(),
            ),
            AppError::NotFoundError { message } => (
                404,
                "NOT_FOUND",
                message.clone(),
            ),
            AppError::InvalidField { field, reason } => (
                400,
                "INVALID_FIELD",
                format!("Invalid field '{}': {}", field, reason),
            ),
            AppError::FileSizeExceeded { size, max } => (
                413,
                "FILE_TOO_LARGE",
                format!("File size {} exceeds maximum allowed {}", size, max),
            ),
            AppError::UploadNotFound { upload_id } => (
                404,
                "UPLOAD_NOT_FOUND",
                format!("Upload not found: {}", upload_id),
            ),
            AppError::UploadAlreadyCompleted { upload_id } => (
                409,
                "UPLOAD_COMPLETED",
                format!("Upload already completed: {}", upload_id),
            ),
            AppError::UploadCancelled { upload_id } => (
                409,
                "UPLOAD_CANCELLED",
                format!("Upload cancelled: {}", upload_id),
            ),
            AppError::InvalidChunkIndex { index } => (
                400,
                "INVALID_CHUNK_INDEX",
                format!("Invalid chunk index: {}", index),
            ),
            AppError::R2Error { message } => (
                502,
                "R2_ERROR",
                format!("Storage error: {}", message),
            ),
            AppError::KvError { message } => (
                502,
                "KV_ERROR",
                format!("Configuration storage error: {}", message),
            ),
            AppError::DatabaseError { message } => (
                502,
                "DATABASE_ERROR",
                message.clone(),
            ),
            AppError::ConfigError { message } => (
                500,
                "CONFIG_ERROR",
                format!("Configuration error: {}", message),
            ),
            AppError::AuthError { message } => (
                401,
                "AUTH_ERROR",
                format!("Authentication error: {}", message),
            ),
            AppError::RateLimitExceeded => (
                429,
                "RATE_LIMIT_EXCEEDED",
                "Rate limit exceeded. Please try again later.".to_string(),
            ),
            AppError::InternalError { message } => (
                500,
                "INTERNAL_ERROR",
                format!("Internal server error: {}", message),
            ),
        };

        let error_response = json!({
            "error": {
                "code": error_code,
                "message": message,
                "timestamp": chrono::Utc::now().to_rfc3339()
            }
        });

        Ok(Response::from_json(&error_response)?.with_status(status))
    }
}

/// Automatic conversion from Cloudflare Worker errors to application errors.
///
/// This implementation provides seamless error conversion from the underlying
/// Cloudflare Workers runtime errors to our structured application errors.
/// It analyzes the error message to determine the appropriate error category.
///
/// # Error Classification
///
/// - **"not found"**: Maps to `DatabaseError`
/// - **"KV" or "kv"**: Maps to `KvError`
/// - **"R2" or "bucket"**: Maps to `R2Error`
/// - **All others**: Maps to `InternalError`
impl From<WorkerError> for AppError {
    fn from(err: WorkerError) -> Self {
        let error_msg = err.to_string();
        
        if error_msg.contains("not found") {
            AppError::DatabaseError {
                message: error_msg.to_string(),
            }
        } else if error_msg.contains("KV") || error_msg.contains("kv") {
            AppError::KvError {
                message: error_msg,
            }
        } else if error_msg.contains("R2") || error_msg.contains("bucket") {
            AppError::R2Error {
                message: error_msg,
            }
        } else {
            AppError::InternalError {
                message: error_msg,
            }
        }
    }
}

/// Type alias for Results using our application error type.
///
/// This provides a convenient shorthand for functions that return
/// results with our custom error type, improving code readability
/// and reducing repetitive type annotations.
///
/// # Example
///
/// ```rust
/// fn validate_upload(data: &str) -> AppResult<UploadMetadata> {
///     // Function implementation
/// }
/// ```
pub type AppResult<T> = std::result::Result<T, AppError>;