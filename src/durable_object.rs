use worker::*;
use std::collections::HashMap;
use crate::models::UploadProgress;
use crate::errors::AppError;
use crate::utils;
use crate::config::Config;
use crate::logging::Logger;
use sha2::{Sha256, Digest};

const MIN_CHUNK_SIZE: usize = 1024 * 1024;
const MAX_CHUNK_SIZE: usize = 10 * 1024 * 1024;
const MAX_PARALLEL_UPLOADS: usize = 5;

#[durable_object]
pub struct UploadTracker {
    state: State,
    env: Env,
    config: Option<Config>,
    logger: Logger,
}

#[durable_object]
impl DurableObject for UploadTracker {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            config: None,
            logger: Logger::new("UploadTracker".to_string()),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        if self.config.is_none() {
            self.config = Some(Config::load(&self.env).await?);
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
    async fn init_upload(&mut self, mut req: Request) -> Result<Response> {
        let form_data = req.form_data().await?;
        let file_name = self.extract_form_field(&form_data, "fileName")?;
        let total_size: usize = self.extract_form_field(&form_data, "totalSize")?.parse()
            .map_err(|e| AppError::BadRequest(format!("Invalid total size: {}", e)))?;

        let upload_id = utils::generate_unique_id();
        let chunk_size = self.calculate_chunk_size(total_size);

        let progress = UploadProgress::new(file_name, total_size, upload_id.clone(), chunk_size);
        self.state.storage().put(&upload_id, progress).await?;

        utils::json_response(&serde_json::json!({
            "uploadId": upload_id,
            "chunkSize": chunk_size,
        }))
    }

    fn calculate_chunk_size(&self, total_size: usize) -> usize {
        let chunk_count = (total_size as f64 / MIN_CHUNK_SIZE as f64).ceil() as usize;
        let chunk_size = total_size / chunk_count;
        chunk_size.clamp(MIN_CHUNK_SIZE, MAX_CHUNK_SIZE)
    }

    async fn upload_chunk(&mut self, mut req: Request) -> Result<Response> {
        let form_data = req.form_data().await?;
        let upload_id = self.extract_form_field(&form_data, "uploadId")?;
        let chunk_index: usize = self.extract_form_field(&form_data, "chunkIndex")?.parse()
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

    async fn combine_chunks(&mut self, progress: &UploadProgress) -> Result<()> {
        self.logger.info(&format!("Combining chunks for file: {}", progress.file_name), None);

        let bucket = self.env.bucket(&self.config.as_ref().unwrap().bucket_name)?;
        let mut hasher = Sha256::new();
        let mut full_file = Vec::with_capacity(progress.total_size);

        for chunk in &progress.chunks {
            let key = format!("{}_chunk_{}", progress.upload_id, chunk.index);
            let chunk_data = self.state.storage().get::<Vec<u8>>(&key).await?;

            full_file.extend_from_slice(&chunk_data);
            hasher.update(&chunk_data);

            self.state.storage().delete(&key).await?;
        }

        let checksum = hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect::<String>();

        let mut metadata = HashMap::new();
        metadata.insert("checksum".to_string(), checksum);

        bucket.put(&progress.file_name, full_file)
            .custom_metadata(metadata)
            .execute()
            .await?;

        self.state.storage().delete(&progress.upload_id).await?;

        self.logger.info(&format!("File {} successfully combined and stored", progress.file_name), None);
        Ok(())
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

    fn extract_form_field(&self, form_data: &FormData, field: &str) -> Result<String> {
        match form_data.get(field) {
            Some(FormEntry::Field(value)) => Ok(value),
            _ => Err(AppError::BadRequest(format!("Invalid {} field", field)).into()),
        }
    }
}