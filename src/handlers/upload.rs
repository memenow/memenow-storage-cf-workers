//! # Upload Handlers
//!
//! End-to-end implementation of the upload lifecycle backed by Cloudflare R2 and D1.
//! The handlers coordinate multipart upload creation, chunk ingestion, completion,
//! and cancellation while keeping metadata in sync with D1.

use chrono::Utc;
use serde::Deserialize;
use uuid::Uuid;
use worker::{HttpMetadata, UploadedPart, *};

use crate::config::Config;
use crate::constants::{MAX_CHUNK_INDEX, STORAGE_BUCKET_NAME};
use crate::database::{DatabaseService, UploadChunkRecord};
use crate::errors::{AppError, AppResult};
use crate::middleware::ValidationMiddleware;
use crate::models::{UploadMetadata, UploadStatus, UserRole};
use crate::utils::generate_r2_key;

#[derive(Debug, Deserialize)]
struct UploadInitRequest {
    file_name: String,
    total_size: u64,
    user_id: String,
    user_role: UserRole,
    #[serde(default = "default_content_type")]
    content_type: String,
}

#[derive(Debug, Deserialize)]
struct UploadLifecycleRequest {
    upload_id: String,
}

fn default_content_type() -> String {
    "application/octet-stream".to_string()
}

/// Initialize a new upload session by creating a multipart upload in R2 and
/// persisting metadata in D1.
pub async fn initialize_upload(
    mut req: Request,
    env: &Env,
    config: &Config,
) -> AppResult<Response> {
    let payload: UploadInitRequest = req.json().await.map_err(|_| AppError::ValidationError {
        message: "Invalid JSON in request body".to_string(),
    })?;

    ValidationMiddleware::validate_file_size(payload.total_size, config.max_file_size)?;
    ValidationMiddleware::validate_content_type(&payload.content_type)?;

    let bucket = env
        .bucket(STORAGE_BUCKET_NAME)
        .map_err(|err| AppError::R2Error {
            message: format!("Unable to access R2 bucket: {err}"),
        })?;

    let database = DatabaseService::new(env, &config.database_name)?;

    let upload_id = Uuid::new_v4().to_string();
    let r2_key = generate_r2_key(
        &payload.user_role,
        &payload.user_id,
        &payload.file_name,
        &payload.content_type,
    );

    let multipart = bucket
        .create_multipart_upload(r2_key.clone())
        .http_metadata(HttpMetadata {
            content_type: Some(payload.content_type.clone()),
            ..Default::default()
        })
        .execute()
        .await
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to initialize multipart upload: {err}"),
        })?;

    let r2_upload_id = multipart.upload_id().await;
    let now = Utc::now();

    let metadata = UploadMetadata {
        upload_id: upload_id.clone(),
        file_name: payload.file_name,
        total_size: payload.total_size,
        created_at: now,
        updated_at: now,
        user_role: payload.user_role,
        content_type: payload.content_type,
        status: UploadStatus::Initiated,
        chunks: Vec::new(),
        r2_key,
        user_id: payload.user_id,
        r2_upload_id,
    };

    database.create_upload(&metadata).await?;

    let body = serde_json::json!({
        "upload_id": metadata.upload_id,
        "chunk_size": config.chunk_size,
        "status": metadata.status.as_str(),
        "r2_key": metadata.r2_key,
    });

    Response::from_json(&body).map_err(|_| AppError::InternalError {
        message: "Failed to serialize upload initialization response".to_string(),
    })
}

/// Upload a single chunk and persist chunk metadata.
pub async fn upload_chunk(mut req: Request, env: &Env, config: &Config) -> AppResult<Response> {
    let (upload_id, chunk_index) = ValidationMiddleware::validate_upload_headers(&req)?;
    if chunk_index > MAX_CHUNK_INDEX {
        return Err(AppError::InvalidChunkIndex { index: chunk_index });
    }

    let chunk_bytes = req.bytes().await.map_err(|err| AppError::ValidationError {
        message: format!("Failed to read chunk body: {err}"),
    })?;

    if chunk_bytes.is_empty() {
        return Err(AppError::ValidationError {
            message: "Chunk body is empty".to_string(),
        });
    }

    let database = DatabaseService::new(env, &config.database_name)?;
    let Some(metadata) = database.get_upload(&upload_id).await? else {
        return Err(AppError::UploadNotFound { upload_id });
    };

    if metadata.status == UploadStatus::Completed {
        return Err(AppError::UploadAlreadyCompleted { upload_id });
    }

    if metadata.status == UploadStatus::Cancelled {
        return Err(AppError::UploadCancelled { upload_id });
    }

    let bucket = env
        .bucket(STORAGE_BUCKET_NAME)
        .map_err(|err| AppError::R2Error {
            message: format!("Unable to access R2 bucket: {err}"),
        })?;

    let multipart = bucket
        .resume_multipart_upload(metadata.r2_key.clone(), metadata.r2_upload_id.clone())
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to resume multipart upload: {err}"),
        })?;

    let part_number = chunk_index + 1;
    let chunk_size = chunk_bytes.len() as u64;

    let uploaded_part = multipart
        .upload_part(part_number, chunk_bytes)
        .await
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to upload chunk to R2: {err}"),
        })?;

    database
        .record_chunk(
            &metadata.upload_id,
            chunk_index,
            chunk_size,
            Some(&uploaded_part.etag()),
        )
        .await?;

    if metadata.status == UploadStatus::Initiated {
        database
            .update_upload_status(&metadata.upload_id, UploadStatus::InProgress)
            .await?;
    } else {
        database.touch_upload(&metadata.upload_id).await?;
    }

    let body = serde_json::json!({
        "upload_id": metadata.upload_id,
        "chunk_index": chunk_index,
        "etag": uploaded_part.etag(),
        "status": UploadStatus::InProgress.as_str(),
    });

    Response::from_json(&body).map_err(|_| AppError::InternalError {
        message: "Failed to serialize chunk upload response".to_string(),
    })
}

/// Complete the multipart upload by stitching R2 parts together.
pub async fn complete_upload(mut req: Request, env: &Env, config: &Config) -> AppResult<Response> {
    let payload: UploadLifecycleRequest =
        req.json().await.map_err(|_| AppError::ValidationError {
            message: "Invalid JSON in request body".to_string(),
        })?;

    let database = DatabaseService::new(env, &config.database_name)?;
    let Some(metadata) = database.get_upload(&payload.upload_id).await? else {
        return Err(AppError::UploadNotFound {
            upload_id: payload.upload_id,
        });
    };

    if metadata.status == UploadStatus::Completed {
        return Err(AppError::UploadAlreadyCompleted {
            upload_id: metadata.upload_id,
        });
    }

    if metadata.status == UploadStatus::Cancelled {
        return Err(AppError::UploadCancelled {
            upload_id: metadata.upload_id,
        });
    }

    let chunk_records = database.get_upload_chunks(&metadata.upload_id).await?;
    if chunk_records.is_empty() {
        return Err(AppError::ValidationError {
            message: "No uploaded chunks to finalize".to_string(),
        });
    }

    let uploaded_parts = build_uploaded_parts(&chunk_records)?;

    let bucket = env
        .bucket(STORAGE_BUCKET_NAME)
        .map_err(|err| AppError::R2Error {
            message: format!("Unable to access R2 bucket: {err}"),
        })?;

    let multipart = bucket
        .resume_multipart_upload(metadata.r2_key.clone(), metadata.r2_upload_id.clone())
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to resume multipart upload: {err}"),
        })?;

    multipart
        .complete(uploaded_parts)
        .await
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to finalize multipart upload: {err}"),
        })?;

    database
        .update_upload_status(&metadata.upload_id, UploadStatus::Completed)
        .await?;

    let body = serde_json::json!({
        "upload_id": metadata.upload_id,
        "status": UploadStatus::Completed.as_str(),
        "r2_key": metadata.r2_key,
    });

    Response::from_json(&body).map_err(|_| AppError::InternalError {
        message: "Failed to serialize completion response".to_string(),
    })
}

/// Cancel an in-flight upload and abort the multipart session.
pub async fn cancel_upload(mut req: Request, env: &Env, config: &Config) -> AppResult<Response> {
    let payload: UploadLifecycleRequest =
        req.json().await.map_err(|_| AppError::ValidationError {
            message: "Invalid JSON in request body".to_string(),
        })?;

    let database = DatabaseService::new(env, &config.database_name)?;
    let Some(metadata) = database.get_upload(&payload.upload_id).await? else {
        return Err(AppError::UploadNotFound {
            upload_id: payload.upload_id,
        });
    };

    if metadata.status == UploadStatus::Completed {
        return Err(AppError::UploadAlreadyCompleted {
            upload_id: metadata.upload_id,
        });
    }

    if metadata.status == UploadStatus::Cancelled {
        return Err(AppError::UploadCancelled {
            upload_id: metadata.upload_id,
        });
    }

    let bucket = env
        .bucket(STORAGE_BUCKET_NAME)
        .map_err(|err| AppError::R2Error {
            message: format!("Unable to access R2 bucket: {err}"),
        })?;

    let multipart = bucket
        .resume_multipart_upload(metadata.r2_key.clone(), metadata.r2_upload_id.clone())
        .map_err(|err| AppError::R2Error {
            message: format!("Failed to resume multipart upload: {err}"),
        })?;

    multipart.abort().await.map_err(|err| AppError::R2Error {
        message: format!("Failed to abort multipart upload: {err}"),
    })?;

    database
        .update_upload_status(&metadata.upload_id, UploadStatus::Cancelled)
        .await?;

    let body = serde_json::json!({
        "upload_id": metadata.upload_id,
        "status": UploadStatus::Cancelled.as_str(),
    });

    Response::from_json(&body).map_err(|_| AppError::InternalError {
        message: "Failed to serialize cancellation response".to_string(),
    })
}

/// Fetch the latest upload status and chunk progress.
pub async fn get_upload_status(req: Request, env: &Env, config: &Config) -> AppResult<Response> {
    let url = req.url().map_err(|err| AppError::InternalError {
        message: format!("Failed to parse request URL: {err}"),
    })?;

    let segments: Vec<&str> = url.path().split('/').collect();
    let upload_id =
        segments
            .iter()
            .rev()
            .skip(1)
            .next()
            .ok_or_else(|| AppError::ValidationError {
                message: "Upload ID missing from path".to_string(),
            })?;

    let database = DatabaseService::new(env, &config.database_name)?;
    let Some(metadata) = database.get_upload(upload_id).await? else {
        return Err(AppError::UploadNotFound {
            upload_id: upload_id.to_string(),
        });
    };

    let body = serde_json::json!({
        "upload_id": metadata.upload_id,
        "status": metadata.status.as_str(),
        "total_size": metadata.total_size,
        "chunks": metadata.chunks,
        "chunk_size": config.chunk_size,
        "r2_key": metadata.r2_key,
        "updated_at": metadata.updated_at.to_rfc3339(),
    });

    Response::from_json(&body).map_err(|_| AppError::InternalError {
        message: "Failed to serialize status response".to_string(),
    })
}

fn build_uploaded_parts(chunks: &[UploadChunkRecord]) -> AppResult<Vec<UploadedPart>> {
    collect_part_descriptors(chunks).map(|descriptors| {
        descriptors
            .into_iter()
            .map(|descriptor| UploadedPart::new(descriptor.part_number, descriptor.etag))
            .collect()
    })
}

#[derive(Debug, PartialEq, Eq)]
struct PartDescriptor {
    part_number: u16,
    etag: String,
}

fn collect_part_descriptors(chunks: &[UploadChunkRecord]) -> AppResult<Vec<PartDescriptor>> {
    let mut parts = Vec::with_capacity(chunks.len());

    for chunk in chunks {
        let Some(etag) = &chunk.etag else {
            return Err(AppError::ValidationError {
                message: format!("Missing ETag for chunk {}", chunk.chunk_index),
            });
        };

        parts.push(PartDescriptor {
            part_number: chunk.chunk_index + 1,
            etag: etag.clone(),
        });
    }

    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::UploadChunkRecord;

    #[test]
    fn collect_part_descriptors_orders_parts_by_index() {
        let chunks = vec![
            UploadChunkRecord {
                chunk_index: 1,
                chunk_size: 10,
                etag: Some("etag-two".into()),
            },
            UploadChunkRecord {
                chunk_index: 0,
                chunk_size: 10,
                etag: Some("etag-one".into()),
            },
        ];

        let parts = collect_part_descriptors(&chunks).unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].part_number, 2);
        assert_eq!(parts[0].etag, "etag-two");
        assert_eq!(parts[1].part_number, 1);
        assert_eq!(parts[1].etag, "etag-one");
    }

    #[test]
    fn collect_part_descriptors_fails_without_etag() {
        let chunks = vec![UploadChunkRecord {
            chunk_index: 0,
            chunk_size: 10,
            etag: None,
        }];

        let error = collect_part_descriptors(&chunks).unwrap_err();
        assert!(matches!(error, AppError::ValidationError { .. }));
    }
}
