//! # Handlers Module
//!
//! This module contains HTTP request handlers for the file storage service.
//! All handlers are organized by functionality and use consistent error handling.

use worker::*;
use std::sync::Arc;

use crate::config::Config;
use crate::utils::cors_headers;

pub mod upload;

/// Handles all upload-related operations using D1 database and R2 storage.
pub async fn handle_upload_routes(req: Request, env: Env, config: Arc<Config>) -> Result<Response> {
    use upload::{
        initialize_upload, upload_chunk, complete_upload, 
        cancel_upload, get_upload_status
    };

    let method = req.method();
    let url = req.url()?;
    let path = url.path();

    let result = match (method, path) {
        (Method::Post, "/api/upload/init") => {
            initialize_upload(req, &env, &config).await
        },
        (Method::Put, "/api/upload/chunk") => {
            upload_chunk(req, &env, &config).await
        },
        (Method::Post, "/api/upload/complete") => {
            complete_upload(req, &env, &config).await
        },
        (Method::Post, "/api/upload/cancel") => {
            cancel_upload(req, &env, &config).await
        },
        (Method::Get, path) if path.starts_with("/api/upload/") && path.ends_with("/status") => {
            get_upload_status(req, &env, &config).await
        },
        _ => {
            return Response::error("Not Found", 404);
        }
    };

    match result {
        Ok(response) => Ok(response.with_headers(cors_headers())),
        Err(app_error) => {
            match app_error.to_response() {
                Ok(response) => Ok(response.with_headers(cors_headers())),
                Err(_) => Response::error("Internal Server Error", 500).map(|r| r.with_headers(cors_headers())),
            }
        }
    }
}

/// Provides a health check endpoint for monitoring and load balancer probes.
pub async fn handle_health_check(_req: Request, _env: Env) -> Result<Response> {
    Response::from_json(&serde_json::json!({
        "status": "healthy",
        "service": "memenow-storage-cf-workers",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handles requests to unmatched routes with a 404 Not Found response.
pub async fn handle_not_found(_req: Request, _env: Env) -> Result<Response> {
    Response::error("Not Found", 404)
}