use serde::{Deserialize, Serialize};
use std::str::FromStr;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum UserRole {
    Creator,
    Member,
    Subscriber,
}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "creator" => Ok(UserRole::Creator),
            "member" => Ok(UserRole::Member),
            "subscriber" => Ok(UserRole::Subscriber),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

impl UserRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserRole::Creator => "creator",
            UserRole::Member => "member",
            UserRole::Subscriber => "subscriber",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum MultipartUploadState {
    NotStarted,
    InProgress(String),
    Completed,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UploadMetadata {
    pub upload_id: String,
    pub file_name: String,
    pub total_size: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_role: String,
    pub content_type: String,
    pub multipart_upload_state: MultipartUploadState,
    pub chunks: Vec<u16>,
    pub r2_key: String,
    pub user_id: String,
}

impl UploadMetadata {
    pub fn new(
        file_name: String,
        total_size: u64,
        upload_id: String,
        user_role: String,
        content_type: String,
        multipart_upload_state: MultipartUploadState,
        chunks: Vec<u16>,
        r2_key: String,
        user_id: String,
    ) -> Self {
        let now = Utc::now();
        Self {
            upload_id,
            file_name,
            total_size,
            created_at: now,
            updated_at: now,
            user_role,
            content_type,
            multipart_upload_state,
            chunks,
            r2_key,
            user_id,
        }
    }
}