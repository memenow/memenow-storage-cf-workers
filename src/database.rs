//! # D1 Database Service
//!
//! This module provides database operations for upload tracking using Cloudflare D1.
//! It replaces the previous Durable Objects implementation with a SQL-based approach
//! for better scalability and query capabilities.
//!
//! ## Core Features
//!
//! - **Upload Metadata Management**: Create, read, update upload records
//! - **Chunk Tracking**: Record individual chunk uploads and progress
//! - **Status Management**: Track upload lifecycle states
//! - **Query Operations**: Support for analytics and dashboards built on top of D1

use chrono::{DateTime, Utc};
use serde::Deserialize;
use worker::{d1::D1Database, wasm_bindgen::JsValue, Env};

use crate::errors::{AppError, AppResult};
use crate::models::{UploadMetadata, UploadStatus, UserRole};

/// D1-backed persistence layer for uploads and chunk metadata.
pub struct DatabaseService {
    db: D1Database,
}

/// Lightweight representation of a stored chunk.
#[derive(Debug, Clone)]
pub struct UploadChunkRecord {
    pub chunk_index: u16,
    pub chunk_size: u64,
    pub etag: Option<String>,
}

impl DatabaseService {
    /// Create a new database service instance for the provided environment.
    pub fn new(env: &Env, binding: &str) -> AppResult<Self> {
        let db = env.d1(binding).map_err(|err| AppError::DatabaseError {
            message: format!("Failed to access D1 binding `{binding}`: {err}"),
        })?;

        Ok(Self { db })
    }

    /// Persist a fresh upload record.
    pub async fn create_upload(&self, metadata: &UploadMetadata) -> AppResult<()> {
        let statement = self.db.prepare(
            "INSERT INTO uploads (
                upload_id,
                file_name,
                total_size,
                content_type,
                user_id,
                user_role,
                r2_key,
                r2_upload_id,
                status,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        );

        let statement = statement
            .bind(&[
                JsValue::from_str(&metadata.upload_id),
                JsValue::from_str(&metadata.file_name),
                JsValue::from_f64(metadata.total_size as f64),
                JsValue::from_str(&metadata.content_type),
                JsValue::from_str(&metadata.user_id),
                JsValue::from_str(metadata.user_role.as_str()),
                JsValue::from_str(&metadata.r2_key),
                JsValue::from_str(&metadata.r2_upload_id),
                JsValue::from_str(metadata.status.as_str()),
                JsValue::from_str(&metadata.created_at.to_rfc3339()),
                JsValue::from_str(&metadata.updated_at.to_rfc3339()),
            ])
            .map_err(map_d1_error("bind insert upload"))?;

        statement
            .run()
            .await
            .map(|_| ())
            .map_err(map_d1_error("insert upload"))
    }

    /// Fetch upload metadata including associated chunk indices.
    pub async fn get_upload(&self, upload_id: &str) -> AppResult<Option<UploadMetadata>> {
        let statement = self
            .db
            .prepare("SELECT * FROM uploads WHERE upload_id = ?1");
        let statement = statement
            .bind(&[JsValue::from_str(upload_id)])
            .map_err(map_d1_error("bind load upload"))?;
        let row: Option<UploadRow> = statement
            .first(None)
            .await
            .map_err(map_d1_error("load upload"))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let chunks = self.fetch_chunks(upload_id).await?;
        let metadata = row.try_into_metadata(chunks)?;

        Ok(Some(metadata))
    }

    /// Update the upload status and timestamp.
    pub async fn update_upload_status(
        &self,
        upload_id: &str,
        status: UploadStatus,
    ) -> AppResult<()> {
        let statement = self.db.prepare(
            "UPDATE uploads
             SET status = ?1, updated_at = ?2
             WHERE upload_id = ?3",
        );

        let statement = statement
            .bind(&[
                JsValue::from_str(status.as_str()),
                JsValue::from_str(&Utc::now().to_rfc3339()),
                JsValue::from_str(upload_id),
            ])
            .map_err(map_d1_error("bind update status"))?;

        statement
            .run()
            .await
            .map(|_| ())
            .map_err(map_d1_error("update upload status"))
    }

    /// Update the last modified timestamp without changing status.
    pub async fn touch_upload(&self, upload_id: &str) -> AppResult<()> {
        let statement = self.db.prepare(
            "UPDATE uploads
             SET updated_at = ?1
             WHERE upload_id = ?2",
        );

        let statement = statement
            .bind(&[
                JsValue::from_str(&Utc::now().to_rfc3339()),
                JsValue::from_str(upload_id),
            ])
            .map_err(map_d1_error("bind touch upload"))?;

        statement
            .run()
            .await
            .map(|_| ())
            .map_err(map_d1_error("touch upload"))
    }

    /// Record or update a chunk row for a multipart upload.
    pub async fn record_chunk(
        &self,
        upload_id: &str,
        chunk_index: u16,
        chunk_size: u64,
        etag: Option<&str>,
    ) -> AppResult<()> {
        let statement = self.db.prepare(
            "INSERT INTO upload_chunks (
                upload_id,
                chunk_index,
                chunk_size,
                etag,
                uploaded_at
            ) VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(upload_id, chunk_index) DO UPDATE SET
                chunk_size = excluded.chunk_size,
                etag = excluded.etag,
                uploaded_at = excluded.uploaded_at",
        );

        let statement = statement
            .bind(&[
                JsValue::from_str(upload_id),
                JsValue::from_f64(chunk_index as f64),
                JsValue::from_f64(chunk_size as f64),
                etag.map_or(JsValue::NULL, JsValue::from_str),
                JsValue::from_str(&Utc::now().to_rfc3339()),
            ])
            .map_err(map_d1_error("bind record chunk"))?;

        statement
            .run()
            .await
            .map(|_| ())
            .map_err(map_d1_error("record chunk"))
    }

    /// Retrieve chunk metadata for an upload.
    pub async fn get_upload_chunks(&self, upload_id: &str) -> AppResult<Vec<UploadChunkRecord>> {
        self.fetch_chunks(upload_id).await
    }

    /// Delete an upload and cascade chunk cleanup.
    pub async fn delete_upload(&self, upload_id: &str) -> AppResult<()> {
        let statement = self.db.prepare("DELETE FROM uploads WHERE upload_id = ?1");
        let statement = statement
            .bind(&[JsValue::from_str(upload_id)])
            .map_err(map_d1_error("bind delete upload"))?;

        statement
            .run()
            .await
            .map(|_| ())
            .map_err(map_d1_error("delete upload"))
    }

    /// List uploads for a given user, optionally filtering by status.
    pub async fn get_user_uploads(
        &self,
        user_id: &str,
        status: Option<UploadStatus>,
    ) -> AppResult<Vec<UploadMetadata>> {
        let (query, bindings): (&str, Vec<JsValue>) = match status {
            Some(status) => (
                "SELECT * FROM uploads WHERE user_id = ?1 AND status = ?2 ORDER BY created_at DESC",
                vec![
                    JsValue::from_str(user_id),
                    JsValue::from_str(status.as_str()),
                ],
            ),
            None => (
                "SELECT * FROM uploads WHERE user_id = ?1 ORDER BY created_at DESC",
                vec![JsValue::from_str(user_id)],
            ),
        };

        let statement = self
            .db
            .prepare(query)
            .bind(&bindings)
            .map_err(map_d1_error("bind list uploads"))?;
        let result = statement
            .all()
            .await
            .map_err(map_d1_error("list uploads"))?;
        let rows: Vec<UploadRow> = result
            .results()
            .map_err(map_d1_error("deserialize uploads"))?;

        let mut uploads = Vec::with_capacity(rows.len());
        for row in rows {
            let chunks = self.fetch_chunks(&row.upload_id).await?;
            uploads.push(row.try_into_metadata(chunks)?);
        }

        Ok(uploads)
    }

    async fn fetch_chunks(&self, upload_id: &str) -> AppResult<Vec<UploadChunkRecord>> {
        let statement = self.db.prepare(
            "SELECT chunk_index, chunk_size, etag
             FROM upload_chunks
             WHERE upload_id = ?1
             ORDER BY chunk_index ASC",
        );

        let statement = statement
            .bind(&[JsValue::from_str(upload_id)])
            .map_err(map_d1_error("bind list chunks"))?;
        let result = statement.all().await.map_err(map_d1_error("list chunks"))?;

        let rows: Vec<ChunkRow> = result
            .results()
            .map_err(map_d1_error("deserialize chunks"))?;

        Ok(rows
            .into_iter()
            .map(|row| UploadChunkRecord {
                chunk_index: row.chunk_index as u16,
                chunk_size: row.chunk_size as u64,
                etag: row.etag,
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct UploadRow {
    upload_id: String,
    file_name: String,
    total_size: f64,
    content_type: String,
    user_id: String,
    user_role: String,
    r2_key: String,
    r2_upload_id: String,
    status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
struct ChunkRow {
    chunk_index: f64,
    chunk_size: f64,
    etag: Option<String>,
}

impl UploadRow {
    fn try_into_metadata(self, chunks: Vec<UploadChunkRecord>) -> AppResult<UploadMetadata> {
        let created_at = DateTime::parse_from_rfc3339(&self.created_at)
            .map_err(|err| AppError::DatabaseError {
                message: format!("Invalid created_at timestamp: {err}"),
            })?
            .with_timezone(&Utc);

        let updated_at = DateTime::parse_from_rfc3339(&self.updated_at)
            .map_err(|err| AppError::DatabaseError {
                message: format!("Invalid updated_at timestamp: {err}"),
            })?
            .with_timezone(&Utc);

        let user_role =
            self.user_role
                .parse::<UserRole>()
                .map_err(|err| AppError::DatabaseError {
                    message: format!("Invalid user_role in database: {err}"),
                })?;

        let status =
            self.status
                .parse::<UploadStatus>()
                .map_err(|err| AppError::DatabaseError {
                    message: format!("Invalid upload status in database: {err}"),
                })?;

        let chunk_indices = chunks.iter().map(|chunk| chunk.chunk_index).collect();

        Ok(UploadMetadata {
            upload_id: self.upload_id,
            file_name: self.file_name,
            total_size: self.total_size as u64,
            created_at,
            updated_at,
            user_role,
            content_type: self.content_type,
            status,
            chunks: chunk_indices,
            r2_key: self.r2_key,
            user_id: self.user_id,
            r2_upload_id: self.r2_upload_id,
        })
    }
}

fn map_d1_error(operation: &'static str) -> impl Fn(worker::Error) -> AppError {
    move |err| AppError::DatabaseError {
        message: format!("{operation} failed: {err}"),
    }
}
