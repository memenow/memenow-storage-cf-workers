//! # MemeNow Storage - Cloudflare Workers
//!
//! Edge file storage service built with Rust on Cloudflare Workers. Provides
//! chunked multipart uploads backed by R2 for object data, D1 for upload
//! metadata, and KV for runtime configuration.
//!
//! ## Modules
//!
//! - `router` — pattern-based HTTP dispatch.
//! - `middleware` — CORS preflight, request validation.
//! - `handlers` — upload lifecycle endpoints and health check.
//! - `database` — D1-backed persistence for upload and chunk records.
//! - `models` — shared types (`UploadMetadata`, `UploadStatus`, `UserRole`).
//! - `config` — KV-loaded configuration with default fallbacks.
//! - `errors` — structured `AppError` to HTTP response mapping.
//! - `utils` — R2 key generation and CORS headers.
//!
//! ## Routes
//!
//! ```text
//! GET  /health                      - Health check
//! POST /api/upload/init             - Initialize a new upload
//! PUT  /api/upload/chunk            - Upload a chunk
//! POST /api/upload/complete         - Finalize the multipart upload
//! POST /api/upload/cancel           - Cancel an in-flight upload
//! GET  /api/upload/{id}/status      - Get upload status
//! ```

use std::sync::{Arc, OnceLock};
use worker::*;

mod config;
mod constants;
mod database;
mod errors;
mod handlers;
mod middleware;
mod models;
mod router;
mod utils;

use config::Config;
use constants::STORAGE_CONFIG_KV_NAME;

static CONFIG_CACHE: OnceLock<Arc<Config>> = OnceLock::new();

/// Worker fetch entry point.
///
/// Installs the panic hook, resolves a cached `Config`, and hands the request
/// to `router::handle_request`. Configuration is cached per worker isolate
/// via `OnceLock`, so the KV round-trip happens at most once per isolate
/// lifetime — not per request.
#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    console_log!("Request: {} {}", req.method(), req.url()?.path());

    let config = load_config(&env).await?;

    router::handle_request(req, env, config).await
}

/// Loads configuration once per isolate and returns the cached `Arc<Config>`.
async fn load_config(env: &Env) -> Result<Arc<Config>> {
    if let Some(config) = CONFIG_CACHE.get() {
        return Ok(config.clone());
    }

    let kv = env.kv(STORAGE_CONFIG_KV_NAME)?;
    let config = Arc::new(Config::load(&kv).await?);
    let _ = CONFIG_CACHE.set(config.clone());
    Ok(config)
}
