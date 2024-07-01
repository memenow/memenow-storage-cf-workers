use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct UploadProgress {
    pub chunks: Vec<ChunkStatus>,
    pub file_name: String,
    pub total_size: usize,
    pub upload_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub chunk_size: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ChunkStatus {
    pub index: usize,
    pub start: usize,
    pub end: usize,
    pub uploaded: bool,
}

impl UploadProgress {
    pub fn new(file_name: String, total_size: usize, upload_id: String, chunk_size: usize) -> Self {
        let now = Utc::now();
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
        Self {
            chunks,
            file_name,
            total_size,
            upload_id,
            created_at: now,
            updated_at: now,
            chunk_size,
        }
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