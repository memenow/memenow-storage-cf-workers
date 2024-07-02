use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum UserRole {
    Creator,
    Member,
    Subscriber,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChunkStatus {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub uploaded: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadProgress {
    pub chunks: Vec<ChunkStatus>,
    pub file_name: String,
    pub total_size: usize,
    pub upload_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub chunk_size: usize,
    pub user_id: String,
    pub user_role: UserRole,
    pub content_type: String,
}

impl UploadProgress {
    pub fn new(
        file_name: String,
        total_size: usize,
        upload_id: String,
        chunk_size: usize,
        user_id: String,
        user_role: UserRole,
        content_type: String,
    ) -> Self {
        let now = Utc::now();
        let chunks = Self::create_chunks(total_size, chunk_size);

        Self {
            chunks,
            file_name,
            total_size,
            upload_id,
            created_at: now,
            updated_at: now,
            chunk_size,
            user_id,
            user_role,
            content_type,
        }
    }

    fn create_chunks(total_size: usize, chunk_size: usize) -> Vec<ChunkStatus> {
        let mut chunks = Vec::new();
        let mut start = 0;
        let mut index = 0;

        while start < total_size {
            let end = (start + chunk_size).min(total_size);
            chunks.push(ChunkStatus {
                index,
                start,
                end,
                uploaded: false,
            });
            start = end;
            index += 1;
        }

        chunks
    }

    pub fn update_chunk(&mut self, chunk_index: usize) {
        if let Some(chunk) = self.chunks.get_mut(chunk_index) {
            chunk.uploaded = true;
            self.updated_at = Utc::now();
        }
    }

    pub fn is_complete(&self) -> bool {
        self.chunks.iter().all(|chunk| chunk.uploaded)
    }
}