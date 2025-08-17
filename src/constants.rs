//! # Application Constants
//!
//! This module defines application-wide constants used throughout the file storage service.
//! Centralizing constants improves maintainability and reduces the risk of inconsistencies
//! across the codebase.
//!
//! ## Binding Names
//!
//! Constants for Cloudflare Worker bindings that must match wrangler.toml configuration.
//!
//! ## Size Limits
//!
//! Default size limits and constraints following industry best practices.
//!
//! ## Headers
//!
//! Standard HTTP header names used by the upload API.

/// Standard KV configuration binding name
pub const STORAGE_CONFIG_KV_NAME: &str = "STORAGE_CONFIG";

/// Standard R2 bucket binding name
pub const STORAGE_BUCKET_NAME: &str = "STORAGE_BUCKET";

/// Standard D1 database binding name for upload tracking
pub const UPLOAD_DB_NAME: &str = "UPLOAD_DB";

/// Default maximum file size (10GB)
pub const DEFAULT_MAX_FILE_SIZE: u64 = 10_737_418_240;

/// Default chunk size (150MB)
pub const DEFAULT_CHUNK_SIZE: u64 = 157_286_400;

/// Maximum reasonable chunk index to prevent abuse
pub const MAX_CHUNK_INDEX: u16 = 10_000;

/// HTTP header for upload session ID
pub const HEADER_UPLOAD_ID: &str = "X-Upload-Id";

/// HTTP header for chunk index
pub const HEADER_CHUNK_INDEX: &str = "X-Chunk-Index";

/// CORS header for allowed origins
pub const CORS_ALLOW_ORIGIN: &str = "*";

/// CORS header for allowed methods
pub const CORS_ALLOW_METHODS: &str = "GET, POST, PUT, DELETE, OPTIONS";

/// CORS header for allowed headers
pub const CORS_ALLOW_HEADERS: &str = "Content-Type, X-Upload-Id, X-Chunk-Index";