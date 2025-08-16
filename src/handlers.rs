//! # HTTP Request Handlers
//!
//! This module contains HTTP request handlers for the file storage service.
//! Handlers are responsible for processing specific types of requests and
//! coordinating with appropriate services (Durable Objects, R2, etc.).
//!
//! ## Handler Categories
//!
//! - **Upload Handlers**: Process file upload operations via Durable Objects
//! - **Health Handlers**: Provide service health and monitoring endpoints
//! - **Error Handlers**: Handle unmatched routes and error conditions
//!
//! ## Design Principles
//!
//! - **Delegation**: Complex logic is delegated to Durable Objects
//! - **CORS Support**: All responses include appropriate CORS headers
//! - **Monitoring**: Health endpoints provide service status information
//! - **Error Handling**: Graceful handling of invalid requests
//!
//! ## Request Flow
//!
//! ```text
//! HTTP Request → Router → Handler → Durable Object → Response
//! ```

use worker::*;
use std::sync::Arc;

use crate::config::Config;
use crate::constants::UPLOAD_TRACKER_INSTANCE;
use crate::utils::cors_headers;

/// Handles all upload-related operations by delegating to Durable Objects.
///
/// This handler serves as a proxy that forwards upload requests to the appropriate
/// Durable Object instance. By using Durable Objects, we ensure that upload state
/// is consistent and persistent across requests, even in a distributed environment.
///
/// # Architecture Benefits
///
/// - **State Consistency**: Durable Objects provide strong consistency guarantees
/// - **Concurrent Safety**: Multiple clients can safely interact with the same upload
/// - **Persistence**: Upload state survives worker restarts and failures
/// - **Scalability**: Each upload gets its own isolated state management
///
/// # Performance Optimizations
///
/// - Uses string slice references to avoid allocations
/// - Reuses CORS headers for better memory efficiency
/// - Minimal request processing before delegation
///
/// # Arguments
///
/// * `req` - The upload request to be processed
/// * `env` - Cloudflare Worker environment for accessing Durable Object bindings
/// * `config` - Configuration containing Durable Object binding name
///
/// # Returns
///
/// Returns a `Result<Response>` containing the processed response with CORS headers.
///
/// # Request Types Handled
///
/// - `POST /v1/uploads/init` - Initialize new upload
/// - `POST /v1/uploads/{id}/chunk` - Upload file chunk
/// - `POST /v1/uploads/{id}/complete` - Complete upload
/// - `GET /v1/uploads/{id}` - Get upload status
/// - `DELETE /v1/uploads/{id}` - Cancel upload
///
/// # Error Handling
///
/// - Durable Object binding errors are propagated as 500 responses
/// - Invalid upload IDs result in 404 responses from the Durable Object
/// - Network errors between Worker and Durable Object are handled gracefully
///
/// # Example Flow
///
/// ```text
/// POST /v1/uploads/init
/// ↓
/// handle_upload_routes()
/// ↓
/// Get Durable Object namespace
/// ↓
/// Create/Get UploadTracker instance
/// ↓
/// Forward request to Durable Object
/// ↓
/// Add CORS headers to response
/// ```
pub async fn handle_upload_routes(req: Request, env: Env, config: Arc<Config>) -> Result<Response> {
    // Get the Durable Object namespace from configuration
    // Use string slice to avoid allocation
    let namespace = env.durable_object(&config.durable_object_name)?;
    
    // Create a consistent ID for the upload tracker instance
    // Using a fixed name ensures all upload operations use the same instance
    let id = namespace.id_from_name(UPLOAD_TRACKER_INSTANCE)?;
    let stub = id.get_stub()?;

    // Forward the request to the Durable Object and await the response
    let do_response = stub.fetch_with_request(req).await?;
    
    // Add CORS headers to support cross-origin requests
    Ok(do_response.with_headers(cors_headers()))
}

/// Provides a health check endpoint for monitoring and load balancer probes.
///
/// This handler returns basic service health information including service
/// identification and current timestamp. It's designed to be lightweight
/// and always return a successful response unless the Worker itself is failing.
///
/// # Arguments
///
/// * `_req` - The health check request (unused)
/// * `_env` - Worker environment (unused)
///
/// # Returns
///
/// Returns a JSON response containing service health information.
///
/// # Response Format
///
/// ```json
/// {
///   "status": "healthy",
///   "service": "memenow-storage-cf-workers",
///   "timestamp": "2024-01-15T10:30:00Z"
/// }
/// ```
///
/// # Use Cases
///
/// - **Load Balancer Probes**: Determines if Worker is accepting traffic
/// - **Monitoring Systems**: Automated health checks and alerting
/// - **Debugging**: Quick verification that the service is running
/// - **Uptime Tracking**: Historical availability monitoring
///
/// # Example Usage
///
/// ```bash
/// curl https://your-worker.example.workers.dev/health
/// ```
pub async fn handle_health_check(_req: Request, _env: Env) -> Result<Response> {
    Response::from_json(&serde_json::json!({
        "status": "healthy",
        "service": "memenow-storage-cf-workers",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

/// Handles requests to unmatched routes with a 404 Not Found response.
///
/// This handler serves as the default fallback for any requests that don't
/// match the defined route patterns. It provides a consistent error response
/// for invalid endpoints.
///
/// # Arguments
///
/// * `_req` - The unmatched request (unused)
/// * `_env` - Worker environment (unused)
///
/// # Returns
///
/// Returns a 404 Not Found error response.
///
/// # Response
///
/// - **Status Code**: 404
/// - **Body**: "Not Found"
/// - **Content-Type**: text/plain
///
/// # Example
///
/// ```bash
/// curl https://your-worker.example.workers.dev/invalid-endpoint
/// # Returns: 404 Not Found
/// ```
///
/// # Future Enhancements
///
/// Consider enhancing this handler to:
/// - Log invalid route attempts for security analysis
/// - Return structured JSON error responses for consistency
/// - Provide helpful suggestions for valid endpoints
pub async fn handle_not_found(_req: Request, _env: Env) -> Result<Response> {
    Response::error("Not Found", 404)
}