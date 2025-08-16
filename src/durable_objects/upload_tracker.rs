//! # Upload Tracker Durable Object
//!
//! This module implements the UploadTracker Durable Object, which provides
//! persistent state management for multipart file uploads. It coordinates
//! upload sessions, tracks progress, and integrates with R2 storage.
//!
//! ## Responsibilities
//!
//! - **Session Management**: Create and track upload sessions
//! - **Progress Tracking**: Monitor chunk upload progress
//! - **R2 Integration**: Coordinate with R2 multipart upload operations  
//! - **State Persistence**: Maintain upload state across worker restarts
//! - **Concurrent Safety**: Handle concurrent chunk uploads safely
//!
//! ## Upload Lifecycle
//!
//! ```text
//! 1. Initialize → Create upload session and R2 multipart upload
//! 2. Upload Chunks → Process individual file chunks
//! 3. Complete → Finalize R2 multipart upload
//! 4. Status Queries → Provide upload progress information
//! 5. Cancellation → Clean up incomplete uploads
//! ```
//!
//! ## Error Handling
//!
//! The Durable Object provides comprehensive error handling for:
//! - Invalid upload states
//! - R2 storage failures
//! - Missing or corrupted metadata
//! - Network failures during operations

use worker::*;
use serde_json::json;
use chrono::Utc;
use std::str::FromStr;

use crate::config::Config;
use crate::constants::{CONFIG_KV_NAME, R2_BUCKET_NAME, MAX_CHUNK_INDEX, DEFAULT_CHUNK_SIZE, HEADER_UPLOAD_ID};
use crate::models::{UploadMetadata, UploadStatus, UserRole};
use crate::utils::*;
use crate::middleware::ValidationMiddleware;


/// Durable Object for managing upload session state and coordinating multipart uploads.
///
/// This Durable Object provides strongly consistent state management for file uploads,
/// ensuring that upload progress is preserved across worker restarts and that
/// concurrent operations on the same upload are handled safely.
///
/// # State Management
///
/// - **Upload Metadata**: Stored in Durable Object storage with upload ID as key
/// - **R2 Integration**: Coordinates with R2 multipart upload operations
/// - **Progress Tracking**: Maintains list of successfully uploaded chunks
/// - **Lifecycle Management**: Tracks upload status from initiation to completion
///
/// # Concurrency
///
/// The Durable Object ensures that:
/// - Only one operation can modify upload state at a time
/// - Chunk uploads can be processed concurrently within the same session
/// - State consistency is maintained across all operations
#[durable_object]
pub struct UploadTracker {
    /// Durable Object state for persistent storage
    state: State,
    /// Worker environment for accessing R2 and KV bindings
    env: Env,
}

impl DurableObject for UploadTracker {
    /// Creates a new UploadTracker instance.
    ///
    /// This constructor is called by the Cloudflare Workers runtime when
    /// a new Durable Object instance is created or when an existing one
    /// is resumed after being dormant.
    ///
    /// # Arguments
    ///
    /// * `state` - Durable Object state for persistent storage
    /// * `env` - Worker environment for accessing bindings
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
        }
    }

    /// Handles incoming HTTP requests to the Durable Object.
    ///
    /// This method serves as the main entry point for all upload operations.
    /// It routes requests to appropriate handlers based on HTTP method and path,
    /// ensuring that each operation is processed with the correct context.
    ///
    /// # Request Routing
    ///
    /// - `POST /v1/uploads/init` → `handle_initiate`: Create new upload session
    /// - `POST /v1/uploads/{id}/chunk` → `handle_upload_chunk`: Upload file chunk
    /// - `POST /v1/uploads/{id}/complete` → `handle_complete`: Complete upload
    /// - `GET /v1/uploads/{id}` → `handle_status`: Get upload status
    /// - `DELETE /v1/uploads/{id}` → `handle_cancel`: Cancel upload
    ///
    /// # Arguments
    ///
    /// * `req` - The incoming HTTP request
    ///
    /// # Returns
    ///
    /// Returns a `Result<Response>` containing the operation result.
    ///
    /// # Error Handling
    ///
    /// - Configuration loading errors are propagated
    /// - Unmatched routes return 404 Not Found
    /// - Handler-specific errors are managed by individual handlers
    async fn fetch(&self, req: Request) -> Result<Response> {
        console_log!("UploadTracker::fetch called");
        console_log!("Request method: {:?}", req.method());
        console_log!("Request path: {:?}", req.url()?.path());

        // Load configuration for upload limits and settings
        let config = self.load_config().await?;

        let url = req.url()?;
        let path = url.path();
        let method = req.method();

        match (method, path) {
            // Initialize new upload session
            (Method::Post, "/v1/uploads/init") => {
                console_log!("Handling initiate request");
                self.handle_initiate(req, &config).await
            },
            // Upload file chunk
            (Method::Post, p) if p.contains("/chunk") => self.handle_upload_chunk(req).await,
            // Complete multipart upload
            (Method::Post, p) if p.contains("/complete") => self.handle_complete(req).await,
            // Get upload status
            (Method::Get, p) if p.starts_with("/v1/uploads/") => self.handle_status(req).await,
            // Cancel upload
            (Method::Delete, p) if p.starts_with("/v1/uploads/") => self.handle_cancel(req).await,
            // Unmatched routes
            _ => {
                console_log!("No matching route found");
                Response::error("Not Found", 404)
            }
        }
    }
}

impl UploadTracker {
    async fn load_config(&self) -> Result<Config> {
        let kv = self.env.kv(CONFIG_KV_NAME)?;
        Config::load(&kv).await
    }

    async fn handle_initiate(&self, mut req: Request, config: &Config) -> Result<Response> {
        console_log!("Handling initiate request");
        
        let body: serde_json::Value = req.json().await
            .map_err(|_| Error::from("Invalid JSON payload"))?;
            
        // Extract and validate required fields
        let file_name = body["fileName"].as_str()
            .ok_or_else(|| Error::from("Missing or invalid fileName"))?
            .to_string();
            
        let total_size = body["totalSize"].as_u64()
            .ok_or_else(|| Error::from("Missing or invalid totalSize"))?;
            
        let user_role = body["userRole"].as_str()
            .ok_or_else(|| Error::from("Missing userRole"))?;
            
        let content_type = body["contentType"].as_str()
            .ok_or_else(|| Error::from("Missing contentType"))?
            .to_string();

        // Validate content type is supported
        if let Err(e) = ValidationMiddleware::validate_content_type(&content_type) {
            return Response::error(&e.to_string(), 400);
        }
            
        let user_id = body["userId"].as_str()
            .ok_or_else(|| Error::from("Missing userId"))?
            .to_string();

        // Validate file size against configured limits
        if let Err(e) = ValidationMiddleware::validate_file_size(total_size, config.max_file_size) {
            return Response::error(&e.to_string(), 413);
        }
        
        // Validate file name isn't empty or malicious
        if file_name.trim().is_empty() || file_name.contains("/") || file_name.contains("..") {
            return Response::error("Invalid file name", 400);
        }

        let upload_id = generate_unique_identifier();
        let metadata = UploadMetadata {
            upload_id: upload_id.clone(),
            file_name,
            total_size,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_role: UserRole::from_str(user_role)
                .map_err(|e| Error::from(format!("Invalid user role: {}", e)))?,
            content_type,
            status: UploadStatus::Initiated,
            chunks: Vec::new(),
            r2_key: generate_r2_key(&body),
            user_id,
            r2_upload_id: String::new(),
        };

        self.state.storage().put(&upload_id, &metadata).await
            .map_err(|e| Error::from(format!("Failed to store metadata: {}", e)))?;

        Response::from_json(&json!({
            "message": "Multipart upload initiated",
            "uploadId": upload_id,
            "r2Key": metadata.r2_key,
        }))
    }

    async fn handle_upload_chunk(&self, mut req: Request) -> Result<Response> {
        console_log!("Handling upload chunk request");
        
        // Validate required headers using middleware
        let (upload_id, chunk_index) = match ValidationMiddleware::validate_upload_headers(&req) {
            Ok(result) => result,
            Err(e) => return Response::error(&e.to_string(), 400),
        };

        // Validate chunk index is reasonable (1-based indexing)
        if chunk_index == 0 || chunk_index > MAX_CHUNK_INDEX {
            return Response::error(&format!("Invalid chunk index: must be between 1 and {}", MAX_CHUNK_INDEX), 400);
        }

        // Retrieve upload metadata
        let mut metadata = match self.state.storage().get::<UploadMetadata>(&upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error("Error retrieving upload metadata", 500);
            }
        };

        // Validate upload is still active
        match metadata.status {
            UploadStatus::Completed => {
                return Response::error("Upload already completed", 409);
            },
            UploadStatus::Cancelled => {
                return Response::error("Upload was cancelled", 409);
            },
            _ => {} // Continue with upload
        }

        // Validate chunk hasn't already been uploaded
        if metadata.chunks.contains(&chunk_index) {
            return Response::error("Chunk already uploaded", 409);
        }

        let chunk_data = req.bytes().await
            .map_err(|_| Error::from("Failed to read chunk data"))?;
            
        // Validate chunk size isn't empty
        if chunk_data.is_empty() {
            return Response::error("Empty chunk data", 400);
        }

        let r2 = self.env.bucket(R2_BUCKET_NAME)
            .map_err(|_| Error::from("Failed to access R2 bucket"))?;

        let multipart_upload = if chunk_index == 1 {
            let new_upload = r2.create_multipart_upload(&metadata.r2_key)
                .execute()
                .await
                .map_err(|e| Error::from(format!("Failed to create multipart upload: {}", e)))?;
            metadata.r2_upload_id = new_upload.upload_id().await.to_string();
            new_upload
        } else {
            if metadata.r2_upload_id.is_empty() {
                return Response::error("Invalid upload state: missing R2 upload ID", 500);
            }
            r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id)
                .map_err(|e| Error::from(format!("Failed to resume multipart upload: {}", e)))?
        };

        let part = multipart_upload.upload_part(chunk_index, chunk_data)
            .await
            .map_err(|e| Error::from(format!("Failed to upload chunk: {}", e)))?;

        // Update metadata
        metadata.chunks.push(chunk_index);
        metadata.chunks.sort_unstable(); // Keep chunks sorted for easier tracking
        metadata.status = UploadStatus::InProgress;
        metadata.updated_at = Utc::now();

        self.state.storage().put(&upload_id, &metadata).await
            .map_err(|e| Error::from(format!("Failed to update metadata: {}", e)))?;

        Response::from_json(&json!({
            "message": "Chunk uploaded successfully",
            "chunkIndex": chunk_index,
            "etag": part.etag(),
            "uploadId": metadata.upload_id,
            "r2UploadId": metadata.r2_upload_id,
            "totalChunks": metadata.chunks.len()
        }))
    }

    async fn handle_complete(&self, mut req: Request) -> Result<Response> {
        console_log!("Handling complete upload request");
        
        let body: serde_json::Value = req.json().await
            .map_err(|_| Error::from("Invalid JSON payload"))?;

        let upload_id = body["uploadId"].as_str()
            .ok_or_else(|| Error::from("Missing uploadId"))?;

        let mut metadata = match self.state.storage().get::<UploadMetadata>(upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error("Error retrieving upload metadata", 500);
            }
        };

        // Validate upload state
        match metadata.status {
            UploadStatus::Completed => {
                return Response::error("Upload already completed", 409);
            },
            UploadStatus::Cancelled => {
                return Response::error("Upload was cancelled", 409);
            },
            UploadStatus::Initiated => {
                return Response::error("Upload has no chunks uploaded yet", 400);
            },
            UploadStatus::InProgress => {} // Continue with completion
        }

        let parts = body["parts"].as_array()
            .ok_or_else(|| Error::from("Missing or invalid 'parts' array"))?;

        if parts.is_empty() {
            return Response::error("No parts provided for completion", 400);
        }

        // Validate and parse parts
        let mut complete_parts = Vec::with_capacity(parts.len());
        for (i, part) in parts.iter().enumerate() {
            let etag = part["etag"].as_str()
                .ok_or_else(|| Error::from(format!("Missing etag for part {}", i + 1)))?;
            let part_number = part["partNumber"].as_u64()
                .ok_or_else(|| Error::from(format!("Missing partNumber for part {}", i + 1)))? as u16;
                
            if etag.is_empty() {
                return Response::error(&format!("Empty etag for part {}", i + 1), 400);
            }
            
            if part_number == 0 {
                return Response::error(&format!("Invalid part number {} for part {}", part_number, i + 1), 400);
            }
            
            complete_parts.push(worker::UploadedPart::new(part_number, etag.to_string()));
        }

        // Verify we have R2 upload ID
        if metadata.r2_upload_id.is_empty() {
            return Response::error("Invalid upload state: missing R2 upload ID", 500);
        }

        let r2 = self.env.bucket(R2_BUCKET_NAME)
            .map_err(|_| Error::from("Failed to access R2 bucket"))?;
            
        let multipart_upload = r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id)
            .map_err(|e| Error::from(format!("Failed to resume multipart upload: {}", e)))?;

        match multipart_upload.complete(complete_parts).await {
            Ok(_) => {
                metadata.status = UploadStatus::Completed;
                metadata.updated_at = Utc::now();
                self.state.storage().put(upload_id, &metadata).await
                    .map_err(|e| Error::from(format!("Failed to update completion status: {}", e)))?;

                Response::from_json(&json!({
                    "message": "Multipart upload completed successfully",
                    "uploadId": upload_id,
                    "r2Key": metadata.r2_key,
                    "totalParts": parts.len()
                }))
            }
            Err(e) => {
                console_error!("Failed to complete multipart upload: {:?}", e);
                Response::error("Failed to complete multipart upload in R2", 500)
            }
        }
    }

    async fn handle_cancel(&self, req: Request) -> Result<Response> {
        console_log!("Handling cancel upload request");
        
        let upload_id = req.headers().get(HEADER_UPLOAD_ID)?
            .ok_or_else(|| Error::from("Missing X-Upload-Id header"))?;

        let mut metadata = match self.state.storage().get::<UploadMetadata>(&upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error("Error retrieving upload metadata", 500);
            }
        };

        // Check if upload is already in a final state
        match metadata.status {
            UploadStatus::Completed => {
                return Response::error("Cannot cancel completed upload", 409);
            },
            UploadStatus::Cancelled => {
                return Response::from_json(&json!({
                    "message": "Upload already cancelled",
                    "uploadId": upload_id
                }));
            },
            _ => {} // Can cancel initiated or in-progress uploads
        }

        // Only abort R2 multipart upload if it was started
        if !metadata.r2_upload_id.is_empty() {
            let r2 = self.env.bucket("BUCKET")
                .map_err(|_| Error::from("Failed to access R2 bucket"))?;
                
            match r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id) {
                Ok(upload) => {
                    if let Err(e) = upload.abort().await {
                        console_error!("Failed to abort R2 multipart upload: {:?}", e);
                        // Continue with cancellation even if R2 abort fails
                    }
                },
                Err(e) => {
                    console_error!("Failed to resume multipart upload for abort: {:?}", e);
                    // Continue with cancellation even if resume fails
                }
            }
        }

        metadata.status = UploadStatus::Cancelled;
        metadata.updated_at = Utc::now();
        
        self.state.storage().put(&upload_id, &metadata).await
            .map_err(|e| Error::from(format!("Failed to update cancellation status: {}", e)))?;

        Response::from_json(&json!({
            "message": "Upload cancelled successfully",
            "uploadId": upload_id
        }))
    }

    async fn handle_status(&self, req: Request) -> Result<Response> {
        console_log!("Handling get upload status request");
        
        let upload_id = req.headers().get(HEADER_UPLOAD_ID)?
            .ok_or_else(|| Error::from("Missing X-Upload-Id header"))?;
            
        let metadata = match self.state.storage().get::<UploadMetadata>(&upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error("Error retrieving upload metadata", 500);
            }
        };

        let progress_percentage = if metadata.total_size > 0 {
            // Estimate progress based on chunks uploaded (rough approximation)
            let estimated_progress = (metadata.chunks.len() as f64 / (metadata.total_size as f64 / DEFAULT_CHUNK_SIZE as f64).ceil()) * 100.0;
            estimated_progress.min(100.0) as u8
        } else {
            0
        };

        Response::from_json(&json!({
            "uploadId": metadata.upload_id,
            "fileName": metadata.file_name,
            "totalSize": metadata.total_size,
            "uploadedChunks": metadata.chunks,
            "totalChunksUploaded": metadata.chunks.len(),
            "status": format!("{:?}", metadata.status),
            "progressPercentage": progress_percentage,
            "createdAt": metadata.created_at.to_rfc3339(),
            "updatedAt": metadata.updated_at.to_rfc3339(),
            "userRole": metadata.user_role.as_str(),
            "contentType": metadata.content_type,
            "r2Key": metadata.r2_key
        }))
    }
}