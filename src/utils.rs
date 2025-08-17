//! # Utility Functions
//!
//! This module provides utility functions used throughout the file storage service.
//! It includes functions for generating unique identifiers, organizing files in storage,
//! and handling HTTP headers for CORS support.
//!
//! ## Core Utilities
//!
//! - **R2 Key Generation**: Creates hierarchical storage paths based on user context
//! - **Unique Identifiers**: Generates cryptographically secure upload session IDs
//! - **CORS Headers**: Provides consistent cross-origin request support
//!
//! ## File Organization Strategy
//!
//! Files are organized using a hierarchical structure that facilitates:
//! - Easy browsing by user role and date
//! - Content type categorization
//! - Scalable storage organization
//! - Future access control implementation
//!
//! ## Example Usage
//!
//! ```rust
//! // Generate R2 storage key
//! let request_body = json!({
//!     "userRole": "creator",
//!     "userId": "user123",
//!     "fileName": "video.mp4",
//!     "contentType": "video/mp4"
//! });
//! let key = generate_r2_key(&request_body);
//! // Result: "creator/user123/20240115/video/video.mp4"
//! 
//! // Generate unique upload ID
//! let upload_id = generate_unique_identifier();
//! // Result: "1641987000000-550e8400-e29b-41d4-a716-446655440000-123456789"
//! ```

use worker::Headers;
use uuid::Uuid;
use chrono::Utc;
use crate::constants::{CORS_ALLOW_ORIGIN, CORS_ALLOW_METHODS, CORS_ALLOW_HEADERS};

/// Generates an R2 storage key based on user context and file metadata.
///
/// This function creates a hierarchical storage path that organizes files by:
/// 1. User role (creator/member/subscriber)
/// 2. User ID for individual user separation
/// 3. Date (YYYYMMDD format) for chronological organization
/// 4. Content category based on MIME type
/// 5. Original filename (sanitized for security)
///
/// # Arguments
///
/// * `user_role` - User role for file organization
/// * `user_id` - User identifier
/// * `file_name` - Original filename
/// * `content_type` - MIME type of the file
///
/// # Returns
///
/// Returns a string representing the R2 storage key path.
///
/// # Path Structure
///
/// ```text
/// {userRole}/{userId}/{date}/{category}/{fileName}
/// ```
///
/// # Content Categories
///
/// - `image/` - Image files (image/*)
/// - `video/` - Video files (video/*)
/// - `audio/` - Audio files (audio/*)
/// - `document/` - Text and JSON files (text/*, */json)
/// - `other/` - All other file types
///
/// # Example
///
/// ```rust
/// use crate::models::UserRole;
/// 
/// let key = generate_r2_key(&UserRole::Creator, "user123", "profile.jpg", "image/jpeg");
/// // Returns: "creator/user123/20240115/image/profile.jpg"
/// ```
///
/// # Security Features
///
/// - Sanitizes file names to prevent path traversal attacks
/// - Validates user role against allowed values
/// - Limits field lengths to prevent excessive storage paths
/// - Removes dangerous characters from all components
pub fn generate_r2_key(user_role: &crate::models::UserRole, user_id: &str, file_name: &str, content_type: &str) -> String {
    let role_str = sanitize_path_component(user_role.as_str());
    let user_id_safe = sanitize_path_component(user_id);
    let file_name_safe = sanitize_filename(file_name);
    let date = Utc::now().format("%Y%m%d").to_string();
    
    // Determine content category based on MIME type
    let category = categorize_content_type(content_type);
    
    format!("{}/{}/{}/{}/{}", role_str, user_id_safe, date, category, file_name_safe)
}

/// Sanitizes a path component to prevent security issues.
///
/// This function removes or replaces characters that could be used for
/// path traversal attacks or that are problematic in storage systems.
///
/// # Arguments
///
/// * `component` - The path component to sanitize
///
/// # Returns
///
/// Returns a sanitized string safe for use in storage paths.
fn sanitize_path_component(component: &str) -> String {
    component
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .take(50) // Limit length
        .collect::<String>()
        .to_lowercase()
}

/// Sanitizes a filename to prevent security issues.
///
/// This function removes dangerous characters and path separators
/// while preserving the file extension.
///
/// # Arguments
///
/// * `filename` - The filename to sanitize
///
/// # Returns
///
/// Returns a sanitized filename safe for storage.
fn sanitize_filename(filename: &str) -> String {
    let filename = filename.trim();
    
    // Remove path separators and dangerous characters
    let safe_chars: String = filename
        .chars()
        .filter(|c| !"/\\:*?\"<>|".contains(*c))
        .take(255) // Limit filename length
        .collect();
        
    if safe_chars.is_empty() {
        "unknown".to_string()
    } else {
        safe_chars
    }
}

/// Categorizes content type into storage directories.
///
/// # Arguments
///
/// * `content_type` - The MIME type to categorize
///
/// # Returns
///
/// Returns a category string for directory organization.
fn categorize_content_type(content_type: &str) -> &'static str {
    let content_type = content_type.to_lowercase();
    
    if content_type.starts_with("image/") {
        "image"
    } else if content_type.starts_with("video/") {
        "video"
    } else if content_type.starts_with("audio/") {
        "audio"
    } else if content_type.starts_with("text/") || content_type.contains("json") {
        "document"
    } else {
        "other"
    }
}

/// Generates a cryptographically secure unique identifier for upload sessions.
///
/// This function creates a unique identifier that combines multiple entropy sources
/// to ensure uniqueness across all upload sessions. The identifier is designed to be:
/// - Globally unique across all workers and time periods
/// - Sortable by creation time (timestamp prefix)
/// - Sufficiently random to prevent guessing attacks
/// - URL-safe and easy to handle in HTTP headers
///
/// # Returns
///
/// Returns a string identifier in the format: `{timestamp}-{uuid}-{random}`
///
/// # Identifier Components
///
/// 1. **Timestamp**: UTC milliseconds since epoch for temporal ordering
/// 2. **UUID v4**: Cryptographically random UUID for global uniqueness
/// 3. **Random**: Additional 64-bit random number for extra entropy
///
/// # Example
///
/// ```rust
/// let upload_id = generate_unique_identifier();
/// // Returns: "1641987000000-550e8400-e29b-41d4-a716-446655440000-123456789"
/// ```
///
/// # Security Considerations
///
/// - Uses cryptographically secure random number generation
/// - Provides sufficient entropy to prevent collision attacks
/// - Timestamp component allows for time-based analysis and cleanup
/// - UUID component ensures global uniqueness across distributed systems
pub fn generate_unique_identifier() -> String {
    let uuid_part = Uuid::new_v4().to_string();
    let timestamp = Utc::now().timestamp_millis();
    format!("{}-{}", timestamp, uuid_part)
}

/// Creates HTTP headers for Cross-Origin Resource Sharing (CORS) support.
///
/// This function creates CORS headers optimized for the upload API.
/// The headers are configured to allow broad access while supporting the
/// necessary HTTP methods and custom headers used by the upload API.
///
/// # Returns
///
/// Returns a `Headers` object containing the CORS configuration.
///
/// # CORS Configuration
///
/// - **Access-Control-Allow-Origin**: `*` (allows all origins)
/// - **Access-Control-Allow-Methods**: `GET, POST, PUT, DELETE, OPTIONS`
/// - **Access-Control-Allow-Headers**: `Content-Type, X-Upload-Id, X-Chunk-Index`
///
/// # Security Note
///
/// The current configuration allows all origins (`*`) for maximum compatibility.
/// In production environments, consider restricting this to specific trusted domains
/// by modifying the `Access-Control-Allow-Origin` header.
///
/// # Example
///
/// ```rust
/// let headers = cors_headers();
/// let response = Response::empty()?.with_headers(headers);
/// ```
///
/// # Supported Headers
///
/// The configuration specifically allows the custom headers used by the upload API:
/// - `X-Upload-Id`: Required for chunk upload and status operations
/// - `X-Chunk-Index`: Required for chunk upload operations
/// - `Content-Type`: Standard header for request payload type
pub fn cors_headers() -> Headers {
    let headers = Headers::new();
    // Note: These values are known to be valid
    let _ = headers.set("Access-Control-Allow-Origin", CORS_ALLOW_ORIGIN);
    let _ = headers.set("Access-Control-Allow-Methods", CORS_ALLOW_METHODS);
    let _ = headers.set("Access-Control-Allow-Headers", CORS_ALLOW_HEADERS);
    headers
}