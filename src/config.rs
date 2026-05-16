//! # Configuration Management
//!
//! Configuration is read from the `STORAGE_CONFIG` KV namespace under the
//! `"config"` key when present, otherwise the service falls back to
//! [`Config::default`].
//!
//! ## Fields
//!
//! - `database_name`: D1 database binding name used by `DatabaseService`.
//! - `max_file_size`: hard cap on `total_size` accepted at upload init (default: 10 GB).
//! - `chunk_size`: recommended chunk size returned to clients (default: 95 MiB, kept under the Workers request body cap).
//!
//! ## Example
//!
//! ```rust
//! let kv = env.kv("STORAGE_CONFIG")?;
//! let config = Config::load(&kv).await?;
//! println!("Max file size: {} bytes", config.max_file_size);
//! ```

use crate::constants::{DEFAULT_CHUNK_SIZE, DEFAULT_MAX_FILE_SIZE, UPLOAD_DB_NAME};
use serde::{Deserialize, Serialize};
use worker::kv::KvStore;
use worker::{console_log, Result};

/// Configuration structure for the file storage service.
///
/// This struct contains all configurable parameters for the service,
/// including upload limits, chunk sizes, and database settings.
/// All fields are public to allow easy access throughout the application.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    /// Name of the D1 database binding used for upload state tracking.
    /// Must match the binding name in wrangler.toml.
    pub database_name: String,

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
    /// - Efficient chunking (95 MiB chunks, under the Workers request body cap)
    /// - Standard D1 database binding name
    fn default() -> Self {
        Self {
            database_name: UPLOAD_DB_NAME.to_string(),
            max_file_size: DEFAULT_MAX_FILE_SIZE,
            chunk_size: DEFAULT_CHUNK_SIZE as usize,
        }
    }
}

impl Config {
    /// Loads configuration from KV storage with fallback to defaults.
    ///
    /// Reads the `"config"` key from KV. Returns [`Config::default`] when the
    /// key is absent. KV access errors and JSON deserialization failures are
    /// propagated.
    ///
    /// Expected KV value:
    ///
    /// ```json
    /// {
    ///   "database_name": "UPLOAD_DB",
    ///   "max_file_size": 10737418240,
    ///   "chunk_size": 99614720
    /// }
    /// ```
    pub async fn load(kv: &KvStore) -> Result<Self> {
        match kv.get("config").json().await? {
            Some(config) => {
                console_log!("Configuration loaded from KV storage");
                Ok(config)
            }
            None => {
                console_log!("Config not found in KV, using default");
                Ok(Self::default())
            }
        }
    }
}
