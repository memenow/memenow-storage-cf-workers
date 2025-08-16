//! # Request Routing and Dispatch
//!
//! This module handles HTTP request routing for the file storage service.
//! It implements a pattern-based router that dispatches requests to appropriate
//! handlers based on HTTP method and URL path patterns.
//!
//! ## Routing Strategy
//!
//! The router uses a simple pattern-matching approach that:
//! - Handles CORS preflight requests automatically
//! - Routes upload operations to Durable Objects for state management
//! - Provides health check endpoints for monitoring
//! - Returns 404 responses for unmatched routes
//!
//! ## Supported Routes
//!
//! - `GET /health` - Health check endpoint
//! - `POST /v1/uploads/*` - Upload-related operations
//! - `GET /v1/uploads/*` - Upload status queries
//! - `DELETE /v1/uploads/*` - Upload cancellation
//! - `OPTIONS *` - CORS preflight requests
//!
//! ## Architecture Benefits
//!
//! - **Centralized Routing**: Single point for request dispatch logic
//! - **CORS Handling**: Automatic handling of cross-origin requests
//! - **Durable Object Integration**: Seamless delegation to stateful handlers
//! - **Extensibility**: Easy to add new route patterns

use worker::*;
use std::sync::Arc;

use crate::config::Config;
use crate::handlers::*;
use crate::middleware::CorsMiddleware;

/// Handles incoming HTTP requests and routes them to appropriate handlers.
///
/// This function serves as the main request dispatcher for the file storage service.
/// It implements a pattern-based routing system that matches HTTP method and path
/// combinations to determine the appropriate handler.
///
/// # Request Flow
///
/// 1. **CORS Preflight**: Handles OPTIONS requests for cross-origin support
/// 2. **Path Extraction**: Extracts URL path and HTTP method from request
/// 3. **Pattern Matching**: Matches against known route patterns
/// 4. **Handler Dispatch**: Delegates to appropriate handler function
/// 5. **Error Handling**: Returns 404 for unmatched routes
///
/// # Arguments
///
/// * `req` - The incoming HTTP request
/// * `env` - Cloudflare Worker environment for accessing bindings
/// * `config` - Shared configuration loaded from KV storage
///
/// # Returns
///
/// Returns a `Result<Response>` containing either the handler response or an error.
///
/// # Route Patterns
///
/// - **Health Check**: `GET /health` → `handle_health_check`
/// - **Upload Operations**: `/v1/uploads/*` → `handle_upload_routes`
/// - **CORS Preflight**: `OPTIONS *` → `CorsMiddleware::handle_preflight`
/// - **Unmatched**: `* *` → `handle_not_found`
///
/// # Example Request Flow
///
/// ```text
/// POST /v1/uploads/init
/// ↓
/// handle_request()
/// ↓
/// handle_upload_routes()
/// ↓
/// Durable Object → UploadTracker
/// ```
///
/// # Error Handling
///
/// - URL parsing errors are propagated up to the main handler
/// - Unmatched routes return 404 Not Found responses
/// - Handler-specific errors are managed by individual handlers
pub async fn handle_request(req: Request, env: Env, config: Arc<Config>) -> Result<Response> {
    // Handle CORS preflight requests early to avoid unnecessary processing
    if req.method() == Method::Options {
        return CorsMiddleware::handle_preflight();
    }

    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    console_log!("Routing request: {} {}", method, path);

    match (method, path) {
        // Health check endpoint for monitoring and load balancer probes
        (Method::Get, "/health") => handle_health_check(req, env).await,
        
        // Upload routes - all upload operations are delegated to Durable Objects
        // This ensures state consistency and proper handling of concurrent operations
        (Method::Post, path) if path.starts_with("/v1/uploads") => {
            handle_upload_routes(req, env, config).await
        },
        (Method::Get, path) if path.starts_with("/v1/uploads") => {
            handle_upload_routes(req, env, config).await
        },
        (Method::Delete, path) if path.starts_with("/v1/uploads") => {
            handle_upload_routes(req, env, config).await
        },
        
        // Default 404 handler for unmatched routes
        _ => handle_not_found(req, env).await,
    }
}