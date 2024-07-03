use worker::*;
use crate::models::{UploadMetadata, MultipartUploadState, UserRole};
use crate::errors::AppError;
use crate::config::Config;
use crate::utils;
use serde_json::json;
use crate::logging::Logger;
use std::str::FromStr;
use async_trait::async_trait;

// Define the struct for the UploadTracker Durable Object
pub struct UploadTracker {
    state: State,
    env: Env,
    config: Config,
    logger: Logger,
}

#[async_trait(?Send)]
impl DurableObject for UploadTracker {
    // Implement the `new` function to initialize the object
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            config: Config::default(),
            logger: Logger::new(utils::generate_request_id()),
        }
    }

    // Implement the `fetch` function to handle requests
    async fn fetch(&mut self, mut req: Request) -> Result<Response> {
        self.logger.info("Durable Object received request", Some(json!({
            "method": req.method().to_string(),
            "url": req.url().map(|u| u.to_string()).unwrap_or_else(|_| "Invalid URL".to_string()),
        })));

        // Load the configuration if it hasn't been loaded yet
        if self.config == Config::default() {
            self.config = Config::load(&self.env).await?;
        }

        // Parse the request body as JSON
        let body: serde_json::Value = req.json().await?;
        let action = body["action"].as_str().ok_or(AppError::BadRequest("Missing action".into()))?;

        self.logger.info("Processing request", Some(json!({ "action": action })));

        // Match the action and call the corresponding method
        let result = match action {
            "initiate" => self.initiate_multipart_upload(&body).await,
            "uploadChunk" => self.handle_chunk_upload(&body, req).await, // Pass `req` as mutable
            "complete" => self.complete_multipart_upload(&body).await,
            "getStatus" => self.get_upload_status(&body).await,
            "cancel" => self.cancel_upload(&body).await,
            _ => Err(AppError::BadRequest("Invalid action".into()).into()),
        };

        // Log any errors that occur during processing
        if let Err(ref e) = result {
            self.logger.error("Error processing request", Some(json!({ "error": e.to_string() })));
        }

        result
    }
}

impl UploadTracker {
    async fn initiate_multipart_upload(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let file_name = body["fileName"].as_str().ok_or(AppError::BadRequest("Missing fileName".into()))?;
        let total_size: u64 = body["totalSize"].as_u64().ok_or(AppError::BadRequest("Invalid totalSize".into()))?;
        let user_role_str = body["userRole"].as_str().ok_or(AppError::BadRequest("Missing userRole".into()))?;
        let content_type = body["contentType"].as_str().ok_or(AppError::BadRequest("Missing contentType".into()))?;

        let user_role = UserRole::from_str(user_role_str).map_err(|e| AppError::BadRequest(e))?;
        if !self.config.is_role_allowed(&user_role) {
            return Err(AppError::Unauthorized("Invalid user role".into()).into());
        }

        if !self.config.has_permission(&user_role, "upload") {
            return Err(AppError::Unauthorized("User does not have upload permission".into()).into());
        }

        let key = utils::generate_r2_key(&self.config, &user_role, upload_id, content_type, file_name);
        self.logger.info("Initiating multipart upload", Some(json!({
            "uploadId": upload_id,
            "fileName": file_name,
            "totalSize": total_size,
            "userRole": user_role_str,
            "contentType": content_type
        })));

        if total_size > self.config.max_file_size {
            return Err(AppError::FileTooLarge("File size exceeds maximum allowed".into()).into());
        }

        let r2 = self.env.bucket(&self.config.bucket_name)?;

        let multipart_upload = r2.create_multipart_upload(&key).execute().await?;
        let r2_upload_id = multipart_upload.upload_id().await;

        let metadata = UploadMetadata::new(
            file_name.to_string(),
            total_size,
            upload_id.to_string(),
            user_role.as_str().to_string(),
            content_type.to_string(),
            MultipartUploadState::InProgress(r2_upload_id.clone()),
            Vec::new(),
            key.clone(),
            upload_id.to_string(),
        );

        self.state.storage().put("metadata", &metadata).await?;

        utils::json_response(&json!({
            "message": "Multipart upload initiated",
            "uploadId": upload_id,
            "r2UploadId": r2_upload_id,
            "key": key,
        }))
    }

    async fn handle_chunk_upload(&self, body: &serde_json::Value, mut req: Request) -> Result<Response> {
        let _upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let chunk_index: u16 = body["chunkIndex"].as_u64().ok_or(AppError::BadRequest("Invalid chunkIndex".into()))? as u16;
        let etag = body["etag"].as_str().ok_or(AppError::BadRequest("Missing etag".into()))?;

        let metadata: Option<UploadMetadata> = self.state.storage().get("metadata").await?;
        let mut metadata = metadata.ok_or(AppError::NotFound("Upload not found".into()))?;

        let _r2_upload_id = match &metadata.multipart_upload_state {
            MultipartUploadState::InProgress(id) => id,
            _ => return Err(AppError::BadRequest("Invalid upload state".into()).into()),
        };

        let r2 = self.env.bucket(&self.config.bucket_name)?;
        let key = format!("{}/{}/{}", metadata.user_role, metadata.content_type, metadata.file_name);

        let chunk_data = req.bytes().await?;
        let part = r2.put(&format!("{}_chunk_{}", key, chunk_index), chunk_data).execute().await?;

        if part.etag() != etag {
            return Err(AppError::BadRequest("ETag mismatch".into()).into());
        }

        metadata.chunks.push(chunk_index);
        self.state.storage().put("metadata", &metadata).await?;

        utils::json_response(&json!({
            "message": "Chunk uploaded successfully",
            "chunkIndex": chunk_index,
            "etag": etag,
        }))
    }

    async fn complete_multipart_upload(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let _parts = body["parts"].as_array().ok_or(AppError::BadRequest("Missing parts".into()))?;

        let metadata: Option<UploadMetadata> = self.state.storage().get("metadata").await?;
        let mut metadata = metadata.ok_or(AppError::NotFound("Upload not found".into()))?;

        let _r2_upload_id = match metadata.multipart_upload_state {
            MultipartUploadState::InProgress(id) => id,
            _ => return Err(AppError::BadRequest("Invalid upload state".into()).into()),
        };

        let r2 = self.env.bucket(&self.config.bucket_name)?;
        let key = format!("{}/{}/{}", metadata.user_role, metadata.content_type, metadata.file_name);

        let mut combined_data = Vec::new();
        for chunk_index in &metadata.chunks {
            let chunk_key = format!("{}_chunk_{}", key, chunk_index);
            let chunk = r2.get(&chunk_key).execute().await?.ok_or(AppError::NotFound("Chunk not found".into()))?;
            let chunk_data = chunk.body().ok_or(AppError::NotFound("Chunk body not found".into()))?.bytes().await?;
            combined_data.extend_from_slice(&chunk_data);
            r2.delete(&chunk_key).await?;
        }

        r2.put(&key, combined_data).execute().await?;

        metadata.multipart_upload_state = MultipartUploadState::Completed;
        self.state.storage().put("metadata", &metadata).await?;

        utils::json_response(&json!({
            "message": "Multipart upload completed successfully",
            "uploadId": upload_id,
        }))
    }

    async fn get_upload_status(&self, body: &serde_json::Value) -> Result<Response> {
        let _upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let metadata: Option<UploadMetadata> = self.state.storage().get("metadata").await?;
        let metadata = metadata.ok_or(AppError::NotFound("Upload not found".into()))?;
        utils::json_response(&metadata)
    }

    async fn cancel_upload(&mut self, body: &serde_json::Value) -> Result<Response> {
        let _upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let metadata: Option<UploadMetadata> = self.state.storage().get("metadata").await?;
        let metadata = metadata.ok_or(AppError::NotFound("Upload not found".into()))?;

        if let MultipartUploadState::InProgress(_) = metadata.multipart_upload_state {
            let r2 = self.env.bucket(&self.config.bucket_name)?;
            let key = format!("{}/{}/{}", metadata.user_role, metadata.content_type, metadata.file_name);
            for chunk_index in &metadata.chunks {
                let chunk_key = format!("{}_chunk_{}", key, chunk_index);
                r2.delete(&chunk_key).await?;
            }
        }

        self.state.storage().delete("metadata").await?;

        utils::json_response(&json!({"message": "Upload cancelled successfully"}))
    }
}
