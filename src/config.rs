//! # Configuration Management
//!
//! This module provides configuration management for the file storage service.
//! Configuration is stored in Cloudflare KV storage and loaded at runtime with
//! intelligent defaults for all required settings.
//!
//! ## Configuration Sources
//!
//! 1. **KV Storage**: Primary configuration source stored under the "config" key
//! 2. **Defaults**: Fallback values when KV storage is unavailable or empty
//!
//! ## Configuration Options
//!
//! - `durable_object_name`: Name of the Durable Object class for upload tracking
//! - `max_file_size`: Maximum allowed file size in bytes (default: 10GB)
//! - `chunk_size`: Size of upload chunks in bytes (default: 150MB)
//!
//! ## Example
//!
//! ```rust
//! let kv = env.kv("CONFIG")?;
//! let config = Config::load(&kv).await?;
//! println!("Max file size: {} bytes", config.max_file_size);
//! ```

use worker::kv::KvStore;
use worker::{console_log, Result};
use serde::{Deserialize, Serialize};
use crate::constants::{UPLOAD_TRACKER_BINDING, DEFAULT_MAX_FILE_SIZE, DEFAULT_CHUNK_SIZE};

/// Configuration structure for the file storage service.
///
/// This struct contains all configurable parameters for the service,
/// including upload limits, chunk sizes, and Durable Object settings.
/// All fields are public to allow easy access throughout the application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Name of the Durable Object binding used for upload state tracking.
    /// Must match the binding name in wrangler.toml.
    pub durable_object_name: String,
    
    /// Maximum allowed file size in bytes.
    /// Files exceeding this limit will be rejected during upload initialization.
    pub max_file_size: u64,
    
    /// Size of individual upload chunks in bytes.
    /// Larger chunks reduce the number of requests but increase memory usage.
    pub chunk_size: usize,
}

impl Default for Config {
    /// Provides default configuration values following industry best practices.
    ///
    /// Default values are optimized for:
    /// - Large file support (up to 10GB)
    /// - Efficient chunking (150MB chunks)
    /// - Standard Durable Object binding name
    fn default() -> Self {
        Self {
            durable_object_name: UPLOAD_TRACKER_BINDING.to_string(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            chunk_size: DEFAULT_CHUNK_SIZE as usize,
        }
    }
}

impl Config {
    /// Loads configuration from KV storage with fallback to defaults.
    ///
    /// This method attempts to load configuration from the "config" key in
    /// KV storage. If no configuration is found or if there's an error
    /// parsing the stored configuration, it falls back to default values.
    ///
    /// # Arguments
    ///
    /// * `kv` - Reference to the KV storage instance
    ///
    /// # Returns
    ///
    /// Returns a `Result<Config>` containing either the loaded configuration
    /// or an error if KV access fails.
    ///
    /// # Example
    ///
    /// ```rust
    /// let kv = env.kv("CONFIG")?;
    /// let config = Config::load(&kv).await?;
    /// ```
    ///
    /// # Configuration Format
    ///
    /// The expected JSON format in KV storage:
    /// ```json
    /// {
    ///   "durable_object_name": "UPLOAD_TRACKER",
    ///   "max_file_size": 10737418240,
    ///   "chunk_size": 157286400
    /// }
    /// ```
    ///
    /// # Error Handling
    ///
    /// - If KV storage is accessible but no config exists, uses defaults
    /// - If KV storage throws an error, the error is propagated up
    /// - Invalid JSON in storage will cause parsing errors
    /// 
    /// # Performance Notes
    ///
    /// Configuration should be loaded once per request and shared via Arc
    /// for optimal performance in high-throughput scenarios.
    pub async fn load(kv: &KvStore) -> Result<Self> {
        match kv.get("config").json().await? {
            Some(config) => {
                console_log!("Configuration loaded from KV storage");
                Ok(config)
            },
            None => {
                console_log!("Config not found in KV, using default");
                Ok(Self::default())
            }
        }
    }
}