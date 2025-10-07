//! # Middleware Components
//!
//! This module provides middleware components for request/response processing
//! in the file storage service. Middleware components handle cross-cutting
//! concerns such as CORS, validation, and error handling.
//!
//! ## Middleware Types
//!
//! - **CORS Middleware**: Handles cross-origin request support
//! - **Validation Middleware**: Validates request headers and parameters
//!
//! ## Design Patterns
//!
//! - **Static Methods**: Middleware functions are implemented as static methods
//! - **Composable**: Middleware can be easily combined and reused
//! - **Error Integration**: Validation middleware integrates with the error system
//! - **Type Safety**: Strong typing for validation results
//!
//! ## Usage Examples
//!
//! ```rust
//! // Apply CORS headers to response
//! let response = CorsMiddleware::apply_headers(response);
//!
//! // Handle CORS preflight
//! if req.method() == Method::Options {
//!     return CorsMiddleware::handle_preflight();
//! }
//!
//! // Validate upload headers
//! let (upload_id, chunk_index) = ValidationMiddleware::validate_upload_headers(&req)?;
//! ```

use crate::constants::{HEADER_CHUNK_INDEX, HEADER_UPLOAD_ID};
use crate::errors::{AppError, AppResult};
use crate::utils::cors_headers;
use worker::*;

/// Middleware for handling Cross-Origin Resource Sharing (CORS) requests.
///
/// This middleware provides CORS support for web applications that need to
/// make cross-origin requests to the file storage service. It handles both
/// preflight requests and applies appropriate headers to responses.
///
/// # CORS Support
///
/// - **Preflight Requests**: Handles OPTIONS requests with proper headers
/// - **Header Application**: Adds CORS headers to all responses
/// - **Broad Compatibility**: Configured for maximum client compatibility
///
/// # Security Considerations
///
/// The current implementation allows all origins (`*`) for maximum compatibility.
/// For production environments with sensitive data, consider restricting origins
/// to specific trusted domains.
pub struct CorsMiddleware;

impl CorsMiddleware {
    /// Applies CORS headers to an existing response.
    ///
    /// This method takes an existing response and adds the necessary CORS
    /// headers to enable cross-origin requests. It's typically called by
    /// handlers to ensure all responses support CORS.
    ///
    /// # Arguments
    ///
    /// * `response` - The response to which CORS headers will be added
    ///
    /// # Returns
    ///
    /// Returns the response with CORS headers applied.
    ///
    /// # Example
    ///
    /// ```rust
    /// let response = Response::from_json(&data)?;
    /// let cors_response = CorsMiddleware::apply_headers(response);
    /// ```
    pub fn apply_headers(response: Response) -> Response {
        response.with_headers(cors_headers())
    }

    /// Handles CORS preflight requests (OPTIONS method).
    ///
    /// Preflight requests are sent by browsers before making cross-origin
    /// requests with certain characteristics. This method returns an empty
    /// response with appropriate CORS headers to indicate that the actual
    /// request is allowed.
    ///
    /// # Returns
    ///
    /// Returns a `Result<Response>` containing an empty response with CORS headers.
    ///
    /// # Example
    ///
    /// ```rust
    /// if req.method() == Method::Options {
    ///     return CorsMiddleware::handle_preflight();
    /// }
    /// ```
    ///
    /// # Browser Behavior
    ///
    /// Browsers send preflight requests for:
    /// - Non-simple HTTP methods (PUT, DELETE, etc.)
    /// - Custom headers (X-Upload-Id, X-Chunk-Index)
    /// - Non-simple content types
    pub fn handle_preflight() -> Result<Response> {
        Ok(Response::empty()?.with_headers(cors_headers()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_file_size_allows_within_limit() {
        assert!(ValidationMiddleware::validate_file_size(1_048_576, 10_485_760).is_ok());
    }

    #[test]
    fn validate_file_size_rejects_over_limit() {
        let err = ValidationMiddleware::validate_file_size(20, 10).unwrap_err();
        assert!(matches!(err, AppError::FileSizeExceeded { .. }));
    }

    #[test]
    fn validate_content_type_accepts_known_prefix() {
        assert!(ValidationMiddleware::validate_content_type("image/png").is_ok());
    }

    #[test]
    fn validate_content_type_rejects_unknown_type() {
        let err =
            ValidationMiddleware::validate_content_type("application/x-msdownload").unwrap_err();
        assert!(matches!(err, AppError::InvalidField { .. }));
    }
}

/// Middleware for validating request parameters and headers.
///
/// This middleware provides validation functions for various aspects of
/// upload requests, including headers, file sizes, and content types.
/// All validation functions integrate with the application error system
/// for consistent error handling.
///
/// # Validation Categories
///
/// - **Header Validation**: Required upload headers (ID, chunk index)
/// - **Size Validation**: File size limits and constraints
/// - **Content Type Validation**: Allowed MIME types and formats
///
/// # Error Integration
///
/// All validation methods return `AppResult<T>` types that integrate
/// seamlessly with the application's error handling system, providing
/// structured error responses to clients.
pub struct ValidationMiddleware;

impl ValidationMiddleware {
    /// Validates and extracts upload-related headers from a request.
    ///
    /// This method validates that the required headers for chunk upload
    /// operations are present and correctly formatted. It's used by
    /// chunk upload endpoints to ensure proper request structure.
    ///
    /// # Required Headers
    ///
    /// - `X-Upload-Id`: Unique identifier for the upload session
    /// - `X-Chunk-Index`: Numeric index of the chunk being uploaded
    ///
    /// # Arguments
    ///
    /// * `req` - The request to validate headers from
    ///
    /// # Returns
    ///
    /// Returns `AppResult<(String, u16)>` containing the upload ID and chunk index,
    /// or an appropriate validation error.
    ///
    /// # Errors
    ///
    /// - `MissingField`: If required headers are not present
    /// - `InvalidField`: If X-Chunk-Index is not a valid number
    ///
    /// # Example
    ///
    /// ```rust
    /// let (upload_id, chunk_index) = ValidationMiddleware::validate_upload_headers(&req)?;
    /// println!("Processing chunk {} for upload {}", chunk_index, upload_id);
    /// ```
    pub fn validate_upload_headers(req: &Request) -> AppResult<(String, u16)> {
        let upload_id = req
            .headers()
            .get(HEADER_UPLOAD_ID)?
            .ok_or(AppError::MissingField {
                field: format!("{} header", HEADER_UPLOAD_ID),
            })?;

        let chunk_index = req
            .headers()
            .get(HEADER_CHUNK_INDEX)?
            .ok_or(AppError::MissingField {
                field: format!("{} header", HEADER_CHUNK_INDEX),
            })?
            .parse::<u16>()
            .map_err(|_| AppError::InvalidField {
                field: HEADER_CHUNK_INDEX.to_string(),
                reason: "Must be a valid number".to_string(),
            })?;

        Ok((upload_id, chunk_index))
    }

    /// Validates that a file size is within configured limits.
    ///
    /// This method checks that the proposed file size does not exceed
    /// the maximum allowed size as configured in the service settings.
    /// It's used during upload initialization to prevent oversized uploads.
    ///
    /// # Arguments
    ///
    /// * `size` - The file size to validate in bytes
    /// * `max_size` - The maximum allowed size in bytes
    ///
    /// # Returns
    ///
    /// Returns `AppResult<()>` indicating success or a size limit error.
    ///
    /// # Errors
    ///
    /// - `FileSizeExceeded`: If the file size exceeds the maximum limit
    ///
    /// # Example
    ///
    /// ```rust
    /// let file_size = 5_000_000_000; // 5GB
    /// let max_size = config.max_file_size;
    /// ValidationMiddleware::validate_file_size(file_size, max_size)?;
    /// ```
    pub fn validate_file_size(size: u64, max_size: u64) -> AppResult<()> {
        if size > max_size {
            return Err(AppError::FileSizeExceeded {
                size,
                max: max_size,
            });
        }
        Ok(())
    }

    /// Validates that a content type is supported by the service.
    ///
    /// This method checks the MIME type of uploaded files against a
    /// whitelist of supported content types. This helps prevent uploads
    /// of potentially dangerous or unsupported file types.
    ///
    /// # Supported Content Types
    ///
    /// - `image/*` - All image formats
    /// - `video/*` - All video formats  
    /// - `audio/*` - All audio formats
    /// - `text/*` - Text files
    /// - `application/json` - JSON documents
    /// - `application/pdf` - PDF documents
    /// - `application/zip` - ZIP archives
    ///
    /// # Arguments
    ///
    /// * `content_type` - The MIME type string to validate
    ///
    /// # Returns
    ///
    /// Returns `AppResult<()>` indicating validation success or failure.
    ///
    /// # Errors
    ///
    /// - `InvalidField`: If the content type is not in the allowed list
    ///
    /// # Example
    ///
    /// ```rust
    /// ValidationMiddleware::validate_content_type("image/jpeg")?; // OK
    /// ValidationMiddleware::validate_content_type("application/exe")?; // Error
    /// ```
    ///
    /// # Security Note
    ///
    /// Content type validation is based on the client-provided MIME type.
    /// For enhanced security, consider implementing file content validation
    /// to verify that the actual file content matches the declared type.
    pub fn validate_content_type(content_type: &str) -> AppResult<()> {
        const ALLOWED_TYPES: &[&str] = &[
            "image/",
            "video/",
            "audio/",
            "text/",
            "application/json",
            "application/pdf",
            "application/zip",
        ];

        if !ALLOWED_TYPES
            .iter()
            .any(|&allowed| content_type.starts_with(allowed))
        {
            return Err(AppError::InvalidField {
                field: "contentType".to_string(),
                reason: "Unsupported file type".to_string(),
            });
        }

        Ok(())
    }
}
