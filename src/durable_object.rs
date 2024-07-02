use worker::*;
use std::collections::HashMap;
use crate::models::{UploadProgress, UserRole};
use crate::errors::AppError;
use crate::utils;
use crate::config::Config;
use crate::logging::Logger;
use sha2::{Sha256, Digest};
use serde_json::Value;

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
            logger: Logger::new("UploadTracker".to_string()),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        if self.config == Config::default() {
            self.config = Config::load(&self.env).await?;
        }

        let url = req.url()?;
        let path = url.path();

        match (req.method(), path) {
            (Method::Post, "/v1/uploads/init") => self.init_upload(req).await,
            (Method::Post, "/v1/uploads/chunk") => self.upload_chunk(req).await,
            (Method::Get, "/v1/uploads") => self.get_progress(req).await,
            (Method::Delete, "/v1/uploads") => self.cancel_upload(req).await,
            _ => Err(AppError::NotFound("Route not found".to_string()).into()),
        }
    }
}

impl UploadTracker {
    async fn init_upload(&self, mut req: Request) -> Result<Response> {
        let body: Value = req.json().await?;
        let file_name = body["fileName"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing fileName".to_string()))?;
        let total_size: usize = body["totalSize"].as_u64()
            .ok_or_else(|| AppError::BadRequest("Invalid totalSize".to_string()))? as usize;
        let user_id = body["userId"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing userId".to_string()))?;
        let user_role = parse_user_role(&body["userRole"])?;
        let content_type = body["contentType"].as_str()
            .ok_or_else(|| AppError::BadRequest("Missing contentType".to_string()))?;

        let upload_id = utils::generate_unique_id();
        let chunk_size = self.calculate_chunk_size(total_size);

        let progress = UploadProgress::new(
            file_name.to_string(),
            total_size,
            upload_id.clone(),
            chunk_size,
            user_id.to_string(),
            user_role,
            content_type.to_string(),
        );
        self.state.storage().put(&upload_id, progress).await?;

        utils::json_response(&serde_json::json!({
            "uploadId": upload_id,
            "chunkSize": chunk_size,
        }))
    }

    async fn upload_chunk(&mut self, mut req: Request) -> Result<Response> {
        let form_data = req.form_data().await?;
        let upload_id = self.extract_form_field(&form_data, "uploadId")?;
        let chunk_index: usize = self.extract_form_field(&form_data, "chunkIndex")?
            .parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid chunk index: {}", e)))?;

        let mut progress = self.state.storage().get::<UploadProgress>(&upload_id).await?;

        let chunk = progress.chunks.get(chunk_index)
            .ok_or_else(|| AppError::BadRequest("Invalid chunk index".to_string()))?;

        let chunk_data = match form_data.get("file") {
            Some(FormEntry::File(file)) => file.bytes().await?,
            _ => return Err(AppError::BadRequest("Invalid file part".to_string()).into()),
        };

        if chunk_data.len() != (chunk.end - chunk.start) {
            return Err(AppError::BadRequest("Chunk size mismatch".to_string()).into());
        }

        let key = format!("{}_chunk_{}", upload_id, chunk_index);
        self.state.storage().put(&key, chunk_data).await?;

        progress.update_chunk(chunk_index);
        self.state.storage().put(&upload_id, &progress).await?;

        let is_complete = progress.is_complete();
        if is_complete {
            self.combine_chunks(&progress).await?;
        }

        utils::json_response(&serde_json::json!({
            "message": "Chunk uploaded successfully",
            "isComplete": is_complete,
        }))
    }

    async fn get_progress(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let query_params = utils::parse_query_string(&url);
        let upload_id = query_params.get("uploadId")
            .ok_or_else(|| AppError::BadRequest("No upload ID in query".to_string()))?;

        let progress = self.state.storage().get::<UploadProgress>(upload_id).await?;

        utils::json_response(&progress)
    }

    async fn cancel_upload(&mut self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let query_params = utils::parse_query_string(&url);
        let upload_id = query_params.get("uploadId")
            .ok_or_else(|| AppError::BadRequest("No upload ID in query".to_string()))?;

        let progress = self.state.storage().get::<UploadProgress>(upload_id).await?;

        for chunk in &progress.chunks {
            let key = format!("{}_chunk_{}", upload_id, chunk.index);
            self.state.storage().delete(&key).await?;
        }

        self.state.storage().delete(upload_id).await?;

        self.logger.info(&format!("Upload cancelled for ID: {}", upload_id), None);
        utils::json_response(&serde_json::json!({"message": "Upload cancelled successfully"}))
    }

    async fn combine_chunks(&self, progress: &UploadProgress) -> Result<()> {
        self.logger.info(&format!("Combining chunks for file: {}", progress.file_name), None);

        let bucket = self.env.bucket(&self.config.bucket_name)?;
        let mut hasher = Sha256::new();
        let full_file = self.collect_chunks(progress, &mut hasher).await?;

        let checksum = hasher.finalize();
        let mut metadata = HashMap::new();
        metadata.insert("checksum".to_string(), hex::encode(checksum));

        let unique_id = utils::generate_unique_id();
        let r2_key = self.create_r2_key(progress, &unique_id);

        bucket.put(&r2_key, full_file)
            .custom_metadata(metadata)
            .execute()
            .await?;

        self.state.storage().delete(&progress.upload_id).await?;

        self.logger.info(&format!("File {} successfully combined and stored as {}", progress.file_name, r2_key), None);
        Ok(())
    }

    async fn collect_chunks(&self, progress: &UploadProgress, hasher: &mut Sha256) -> Result<Vec<u8>> {
        let mut full_file = Vec::with_capacity(progress.total_size);

        for chunk in &progress.chunks {
            let key = format!("{}_chunk_{}", progress.upload_id, chunk.index);
            let chunk_data = self.state.storage().get::<Vec<u8>>(&key).await?;

            full_file.extend_from_slice(&chunk_data);
            hasher.update(&chunk_data);

            self.state.storage().delete(&key).await?;
        }

        Ok(full_file)
    }

    fn create_r2_key(&self, progress: &UploadProgress, unique_id: &str) -> String {
        format!("{}/{}/{}/{}_{}",
                match progress.user_role {
                    UserRole::Creator => "creators",
                    UserRole::Member => "members",
                    UserRole::Subscriber => "subscribers",
                },
                progress.user_id,
                progress.content_type,
                unique_id,
                progress.file_name
        )
    }

    fn calculate_chunk_size(&self, total_size: usize) -> usize {
        let chunk_count = (total_size as f64 / self.config.min_chunk_size as f64).ceil() as usize;
        let chunk_size = total_size / chunk_count;
        chunk_size.clamp(self.config.min_chunk_size, self.config.max_chunk_size)
    }

    fn extract_form_field(&self, form_data: &FormData, field: &str) -> Result<String> {
        match form_data.get(field) {
            Some(FormEntry::Field(value)) => Ok(value),
            _ => Err(AppError::BadRequest(format!("Invalid {} field", field)).into()),
        }
    }
}

fn parse_user_role(value: &Value) -> Result<UserRole> {
    match value.as_str().ok_or_else(|| AppError::BadRequest("Missing userRole".to_string()))? {
        "Creator" => Ok(UserRole::Creator),
        "Member" => Ok(UserRole::Member),
        "Subscriber" => Ok(UserRole::Subscriber),
        _ => Err(AppError::BadRequest("Invalid userRole".to_string()).into()),
    }
}