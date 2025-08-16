//! # Data Models and Types
//!
//! This module defines the core data structures used throughout the file storage service.
//! All models are designed to be serializable for storage in Durable Objects and
//! network transmission via JSON APIs.
//!
//! ## Core Types
//!
//! - `UserRole`: Enumeration of user roles for file organization
//! - `UploadMetadata`: Complete metadata for an upload session
//! - `UploadStatus`: State tracking for upload progress
//!
//! ## Design Principles
//!
//! - All types implement `Serialize` and `Deserialize` for JSON compatibility
//! - Enums use string representations for API clarity
//! - Metadata includes comprehensive tracking information
//! - UTC timestamps for global consistency

use serde::{Deserialize, Serialize};
use std::str::FromStr;
use chrono::{DateTime, Utc};

/// User role enumeration for file organization and access control.
///
/// User roles determine how files are organized in R2 storage and may be used
/// for future access control features. Each role corresponds to a different
/// directory structure in the storage bucket.
///
/// # Storage Organization
///
/// Files are organized by role as follows:
/// - `Creator`: `/creator/{user_id}/{date}/{category}/{filename}`
/// - `Member`: `/member/{user_id}/{date}/{category}/{filename}`
/// - `Subscriber`: `/subscriber/{user_id}/{date}/{category}/{filename}`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserRole {
    /// Content creator with full upload privileges
    Creator,
    /// Regular member with standard upload access
    Member,
    /// Subscriber with limited upload capabilities
    Subscriber,
}

impl UserRole {
    /// Converts the user role to its string representation.
    ///
    /// This method returns the lowercase string representation used in
    /// storage paths and API responses.
    ///
    /// # Returns
    ///
    /// Returns a static string slice representing the role.
    ///
    /// # Example
    ///
    /// ```rust
    /// assert_eq!(UserRole::Creator.as_str(), "creator");
    /// assert_eq!(UserRole::Member.as_str(), "member");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Creator => "creator",
            UserRole::Member => "member",
            UserRole::Subscriber => "subscriber",
        }
    }
}

impl FromStr for UserRole {
    type Err = String;

    /// Parses a string into a UserRole variant.
    ///
    /// The parsing is case-insensitive and accepts the standard role names.
    /// Invalid role names return a descriptive error message.
    ///
    /// # Arguments
    ///
    /// * `s` - String slice to parse
    ///
    /// # Returns
    ///
    /// Returns `Ok(UserRole)` for valid roles or `Err(String)` with error message.
    ///
    /// # Example
    ///
    /// ```rust
    /// use std::str::FromStr;
    /// 
    /// let role = UserRole::from_str("CREATOR").unwrap();
    /// assert_eq!(role, UserRole::Creator);
    /// ```
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "creator" => Ok(UserRole::Creator),
            "member" => Ok(UserRole::Member),
            "subscriber" => Ok(UserRole::Subscriber),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

/// Complete metadata for an upload session.
///
/// This structure contains all information needed to track and manage
/// a multipart file upload. It is stored in Durable Objects and updated
/// throughout the upload lifecycle.
///
/// # Fields
///
/// - `upload_id`: Unique identifier for the upload session
/// - `file_name`: Original filename provided by the client
/// - `total_size`: Total file size in bytes
/// - `created_at`: UTC timestamp when upload was initiated
/// - `updated_at`: UTC timestamp of last metadata update
/// - `user_role`: User's role for file organization
/// - `content_type`: MIME type of the uploaded file
/// - `status`: Current upload status
/// - `chunks`: List of successfully uploaded chunk indices
/// - `r2_key`: R2 storage key for the file
/// - `user_id`: Identifier of the uploading user
/// - `r2_upload_id`: R2 multipart upload identifier
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadMetadata {
    /// Unique identifier for this upload session.
    /// Generated using UUID v4 with timestamp and random components.
    pub upload_id: String,
    
    /// Original filename as provided by the client.
    /// Used in the final R2 storage path.
    pub file_name: String,
    
    /// Total size of the file being uploaded in bytes.
    /// Validated against configured maximum file size.
    pub total_size: u64,
    
    /// UTC timestamp when the upload was initiated.
    /// Used for tracking upload duration and cleanup.
    pub created_at: DateTime<Utc>,
    
    /// UTC timestamp of the last metadata update.
    /// Updated whenever upload progress changes.
    pub updated_at: DateTime<Utc>,
    
    /// User role determining file organization structure.
    pub user_role: UserRole,
    
    /// MIME content type of the uploaded file.
    /// Used for content category determination and validation.
    pub content_type: String,
    
    /// Current status of the upload operation.
    pub status: UploadStatus,
    
    /// Vector of chunk indices that have been successfully uploaded.
    /// Used to track progress and handle resumable uploads.
    pub chunks: Vec<u16>,
    
    /// R2 storage key where the file will be stored.
    /// Generated based on user role, ID, date, and content type.
    pub r2_key: String,
    
    /// Identifier of the user performing the upload.
    pub user_id: String,
    
    /// R2 multipart upload identifier.
    /// Required for completing the multipart upload operation.
    pub r2_upload_id: String,
}

/// Upload status enumeration tracking the lifecycle of an upload.
///
/// The upload status progresses through these states:
/// 1. `Initiated` - Upload session created but no chunks uploaded
/// 2. `InProgress` - One or more chunks have been uploaded
/// 3. `Completed` - All chunks uploaded and multipart upload completed
/// 4. `Cancelled` - Upload was cancelled and R2 multipart upload aborted
///
/// # State Transitions
///
/// ```text
/// Initiated -> InProgress -> Completed
///     |             |
///     v             v
/// Cancelled <- Cancelled
/// ```
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum UploadStatus {
    /// Upload session has been created but no chunks have been uploaded yet.
    Initiated,
    
    /// Upload is in progress with one or more chunks successfully uploaded.
    InProgress,
    
    /// Upload has been completed successfully and file is available in R2.
    Completed,
    
    /// Upload has been cancelled and any uploaded chunks have been cleaned up.
    Cancelled,
}