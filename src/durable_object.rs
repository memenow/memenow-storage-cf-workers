use worker::*;
use serde_json::json;
use crate::models::{UploadMetadata, MultipartUploadState, UserRole};
use crate::errors::AppError;
use crate::config::Config;
use crate::utils;
use crate::logging::Logger;
use std::str::FromStr;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;

#[durable_object]
pub struct UploadTracker {
    state: State,
    env: Env,
    config: Config,
    logger: Logger,
}

#[durable_object]
impl DurableObject for UploadTracker {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            config: Config::default(),
            logger: Logger::new(utils::generate_request_id()),
        }
    }

    async fn fetch(&mut self, mut req: Request) -> Result<Response> {
        if self.config == Config::default() {
            self.config = Config::load(&self.env).await?;
        }
    
        let body = req.text().await?;
        let body: serde_json::Value = serde_json::from_str(&body)?;
        let action = body["action"].as_str().ok_or(AppError::BadRequest("Missing action".into()))?;
    
        self.logger.info("Processing request", Some(json!({ "action": action })));
    
        let result = match action {
            "initiate" => self.initiate_multipart_upload(&body).await,
            "uploadChunk" => self.handle_chunk_upload(&body).await,
            "complete" => self.complete_multipart_upload(&body).await,
            "getStatus" => self.get_upload_status(&body).await,
            "cancel" => self.cancel_upload(&body).await,
            _ => Err(AppError::BadRequest("Invalid action".into()).into()),
        };
    
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
        let user_id = body["userId"].as_str().ok_or(AppError::BadRequest("Missing userId".into()))?;
        let content_type = body["contentType"].as_str().ok_or(AppError::BadRequest("Missing contentType".into()))?;
    
        let user_role = UserRole::from_str(user_role_str).map_err(|e| AppError::BadRequest(e))?;
    
        let r2_key = utils::generate_r2_key(&self.config, &user_role, user_id, content_type, file_name);
    
        self.logger.info("Initiating multipart upload", Some(json!({
            "uploadId": upload_id,
            "fileName": file_name,
            "totalSize": total_size,
            "userRole": user_role,
            "contentType": content_type,
            "r2Key": r2_key
        })));
    
        let r2 = self.env.bucket(&self.config.bucket_name)?;
    
        let multipart_upload = r2.create_multipart_upload(&r2_key)
            .execute()
            .await?;
    
        let r2_upload_id = multipart_upload.upload_id().await;
    
        let metadata = UploadMetadata::new(
            file_name.to_string(),
            total_size,
            upload_id.to_string(),
            user_role.as_str().to_string(),
            content_type.to_string(),
            MultipartUploadState::InProgress(r2_upload_id.clone()),
            Vec::new(),
            r2_key.clone(),
            user_id.to_string(),
        );
    
        self.state.storage().put("metadata", &metadata).await?;
    
        utils::json_response(&json!({
            "message": "Multipart upload initiated",
            "uploadId": upload_id,
            "r2UploadId": r2_upload_id,
            "key": r2_key,
        }))
    }
    
    async fn handle_chunk_upload(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let chunk_index: u16 = body["chunkIndex"].as_u64().ok_or(AppError::BadRequest("Invalid chunkIndex".into()))? as u16;
        let chunk_data_base64 = body["chunkData"].as_str().ok_or(AppError::BadRequest("Missing chunkData".into()))?;
        let etag = body["etag"].as_str().ok_or(AppError::BadRequest("Missing etag".into()))?;
    
        let metadata: UploadMetadata = self.state.storage().get::<UploadMetadata>("metadata").await?;
    
        let r2_upload_id = match &metadata.multipart_upload_state {
            MultipartUploadState::InProgress(id) => id.clone(),
            _ => return Err(AppError::BadRequest("Invalid upload state".into()).into()),
        };
    
        let r2 = self.env.bucket(&self.config.bucket_name)?;
    
        let chunk_data = BASE64.decode(chunk_data_base64).map_err(|_| AppError::BadRequest("Invalid chunk data".into()))?;

        let multipart_upload = r2.resume_multipart_upload(&metadata.r2_key, &r2_upload_id);
        let part = multipart_upload?.upload_part(chunk_index, chunk_data).await?;
    
        if part.etag() != etag {
            return Err(AppError::BadRequest("ETag mismatch".into()).into());
        }
    
        let mut updated_metadata = metadata;
        updated_metadata.chunks.push(chunk_index);
        self.state.storage().put("metadata", &updated_metadata).await?;
    
        utils::json_response(&json!({
            "message": "Chunk uploaded successfully",
            "chunkIndex": chunk_index,
            "etag": etag,
        }))
    }

    async fn complete_multipart_upload(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let parts = body["parts"].as_array().ok_or(AppError::BadRequest("Missing parts".into()))?;
    
        let metadata: UploadMetadata = self.state.storage().get::<UploadMetadata>("metadata").await?;
    
        let r2_upload_id = match &metadata.multipart_upload_state {
            MultipartUploadState::InProgress(id) => id.clone(),
            _ => return Err(AppError::BadRequest("Invalid upload state".into()).into()),
        };
    
        let r2 = self.env.bucket(&self.config.bucket_name)?;
    
        let complete_parts: Vec<UploadedPart> = parts.iter().map(|part| {
            let etag = part["etag"].as_str().unwrap();
            let part_number = part["partNumber"].as_u64().unwrap() as u16;
            UploadedPart::new(part_number, etag.to_string())
        }).collect();
    
        let multipart_upload = r2.resume_multipart_upload(&metadata.r2_key, &r2_upload_id);
        multipart_upload?.complete(complete_parts).await?;
    
        let mut updated_metadata = metadata;
        updated_metadata.multipart_upload_state = MultipartUploadState::Completed;
        self.state.storage().put("metadata", &updated_metadata).await?;
    
        utils::json_response(&json!({
            "message": "Multipart upload completed successfully",
            "uploadId": upload_id,
        }))
    }

    async fn get_upload_status(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;

        let metadata: UploadMetadata = self.state.storage().get("metadata").await?;

        let status = json!({
            "uploadId": metadata.upload_id,
            "fileName": metadata.file_name,
            "totalSize": metadata.total_size,
            "uploadedChunks": metadata.chunks,
            "status": match metadata.multipart_upload_state {
                MultipartUploadState::NotStarted => "not_started",
                MultipartUploadState::InProgress(_) => "in_progress",
                MultipartUploadState::Completed => "completed",
            },
        });

        utils::json_response(&status)
    }

    async fn cancel_upload(&self, body: &serde_json::Value) -> Result<Response> {
        let upload_id = body["uploadId"].as_str().ok_or(AppError::BadRequest("Missing uploadId".into()))?;
        let metadata: UploadMetadata = self.state.storage().get::<UploadMetadata>("metadata").await?;

        if let MultipartUploadState::InProgress(r2_upload_id) = metadata.multipart_upload_state {
            let r2 = self.env.bucket(&self.config.bucket_name)?;
            let multipart_upload = r2.resume_multipart_upload(&metadata.r2_key, &r2_upload_id);
            multipart_upload?.abort().await?;
        }

        self.state.storage().delete("metadata").await?;

        utils::json_response(&json!({"message": "Upload cancelled successfully"}))
    }
}