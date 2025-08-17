//! # MemeNow Storage - Cloudflare Workers
//!
//! A high-performance file storage service built with Rust and Cloudflare Workers.
//! This service provides robust chunked upload capabilities for large files using
//! R2 storage, D1 database for state management, and KV storage for configuration.
//!
//! ## Architecture
//!
//! The service follows a modular architecture with clear separation of concerns:
//! - **Router**: Routes incoming requests to appropriate handlers
//! - **Middleware**: Handles CORS, validation, and error processing
//! - **Handlers**: Process business logic for upload operations
//! - **Database**: Manage upload state persistence with D1 SQL database
//! - **Models**: Define data structures and types
//! - **Utils**: Provide utility functions for file organization and ID generation
//!
//! ## Core Features
//!
//! - Chunked file uploads supporting files up to 10GB
//! - Role-based file organization (creator/member/subscriber)
//! - Multipart upload state management via D1 Database
//! - Comprehensive error handling with structured responses
//! - Configurable upload limits and chunk sizes
//! - CORS support for web applications
//!
//! ## Example Usage
//!
//! The service exposes a REST API for file upload operations:
//!
//! ```text
//! POST /api/upload/init             - Initialize a new upload
//! PUT  /api/upload/chunk            - Upload a file chunk
//! POST /api/upload/complete         - Complete the upload
//! GET  /api/upload/{id}/status      - Get upload status
//! POST /api/upload/cancel           - Cancel an upload
//! ```

use worker::*;
use std::sync::Arc;

mod config;
mod constants;
mod models;
mod utils;
mod database;
mod handlers;
mod router;
mod errors;
mod middleware;

use config::Config;
use constants::STORAGE_CONFIG_KV_NAME;

/// Main entry point for the Cloudflare Worker.
///
/// This function serves as the primary request handler that:
/// 1. Sets up panic handling for better debugging
/// 2. Loads configuration from KV storage with fallback to defaults
/// 3. Delegates request routing to the router module
///
/// # Arguments
///
/// * `req` - The incoming HTTP request
/// * `env` - Cloudflare Worker environment providing access to bindings
/// * `_ctx` - Request context (unused in current implementation)
///
/// # Returns
///
/// Returns a `Result<Response>` containing either the HTTP response or an error.
///
/// # Error Handling
///
/// All errors are handled gracefully and converted to appropriate HTTP responses
/// with structured error messages and proper status codes.
///
/// # Performance Considerations
///
/// - Configuration is loaded once per request and shared via Arc for efficiency
/// - Request logging is minimal to reduce overhead
/// - Panic hook is set only once globally
/// - CORS headers are created per request for thread safety in WASM environment
#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    // Set up panic hook for better error reporting in development
    console_error_panic_hook::set_once();

    console_log!("Request: {} {}", req.method(), req.url()?.path());

    // Load configuration from KV storage with fallback to defaults
    let kv = env.kv(STORAGE_CONFIG_KV_NAME)?;
    let config = Arc::new(Config::load(&kv).await?);

    // Route the request to appropriate handlers
    router::handle_request(req, env, config).await
}