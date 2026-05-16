//! # Request Routing
//!
//! Pattern-based dispatcher for the file storage service. Matches HTTP method
//! and path against a fixed route table and forwards to the appropriate
//! handler. Handles CORS preflight early so `OPTIONS` never reaches a handler.
//!
//! ## Supported Routes
//!
//! - `GET /health` — health check
//! - `POST /api/upload/init` — initialize an upload
//! - `PUT  /api/upload/chunk` — upload a chunk
//! - `POST /api/upload/complete` — finalize the multipart upload
//! - `POST /api/upload/cancel` — cancel an upload
//! - `GET  /api/upload/{id}/status` — get upload status
//! - `OPTIONS *` — CORS preflight

use std::sync::Arc;
use worker::*;

use crate::config::Config;
use crate::handlers::{handle_health_check, handle_not_found, handle_upload_routes};
use crate::middleware::CorsMiddleware;

/// Dispatches an incoming request to the appropriate handler.
///
/// CORS preflight is short-circuited before any path matching. Anything under
/// `/api/upload` is delegated to [`handle_upload_routes`]; unmatched routes
/// return 404 via [`handle_not_found`].
pub async fn handle_request(req: Request, env: Env, config: Arc<Config>) -> Result<Response> {
    if req.method() == Method::Options {
        return CorsMiddleware::handle_preflight();
    }

    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    console_log!("Routing request: {} {}", method, path);

    match (method, path) {
        (Method::Get, "/health") => handle_health_check(req, env).await,

        (Method::Post, path) if path.starts_with("/api/upload") => {
            handle_upload_routes(req, env, config).await
        }
        (Method::Put, path) if path.starts_with("/api/upload") => {
            handle_upload_routes(req, env, config).await
        }
        (Method::Get, path) if path.starts_with("/api/upload") => {
            handle_upload_routes(req, env, config).await
        }

        _ => handle_not_found(req, env).await,
    }
}
