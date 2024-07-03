use worker::*;
use std::sync::Arc;
use once_cell::sync::OnceCell;
use crate::models::UserRole;
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub durable_object_name: String,
    pub tracker_name: String,
    pub bucket_name: String,
    pub max_file_size: u64,
    pub rate_limit: u32,
    pub min_chunk_size: usize,
    pub max_chunk_size: usize,
    pub max_parallel_uploads: usize,
    pub allowed_roles: Vec<UserRole>,
    pub role_permissions: HashMap<UserRole, Vec<String>>,
    pub path_format: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            durable_object_name: "UPLOAD_TRACKER".to_string(),
            tracker_name: "tracker".to_string(),
            bucket_name: "BUCKET".to_string(),
            max_file_size: 10_737_418_240,
            rate_limit: 100,
            min_chunk_size: 51200,
            max_chunk_size: 131072,
            max_parallel_uploads: 5,
            allowed_roles: vec![UserRole::Creator, UserRole::Member, UserRole::Subscriber],
            role_permissions: HashMap::new(),
            path_format: "{role}/{user_id}/content/{date}/{content_type}/{unique_id}.{extension}".to_string(),
        }
    }
}

static CONFIG: OnceCell<Arc<Config>> = OnceCell::new();

impl Config {
    pub async fn load(env: &Env) -> Result<Config> {
        if let Some(config) = CONFIG.get() {
            return Ok(config.as_ref().clone());
        }

        let kv = env.kv("CONFIG")?;

        let allowed_roles_str = kv.get("ALLOWED_ROLES").text().await?.unwrap_or_else(|| "creator,member,subscriber".to_string());
        let allowed_roles = allowed_roles_str.split(',')
            .filter_map(|role| UserRole::from_str(role.trim()).ok())
            .collect::<Vec<UserRole>>();

        let mut role_permissions = HashMap::new();
        for role in &allowed_roles {
            let permissions_key = format!("{}_PERMISSIONS", role.as_str().to_uppercase());
            let permissions = kv.get(&permissions_key).text().await?
                .unwrap_or_else(|| "upload,download".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();
            role_permissions.insert(role.clone(), permissions);
        }

        let config = Config {
            durable_object_name: kv.get("DURABLE_OBJECT_NAME").text().await?.unwrap_or_else(|| "UPLOAD_TRACKER".to_string()),
            tracker_name: kv.get("TRACKER_NAME").text().await?.unwrap_or_else(|| "tracker".to_string()),
            bucket_name: kv.get("BUCKET_NAME").text().await?.unwrap_or_else(|| "BUCKET".to_string()),
            max_file_size: kv.get("MAX_FILE_SIZE").text().await?.unwrap_or_else(|| "10737418240".to_string()).parse().unwrap_or(10_737_418_240),
            rate_limit: kv.get("RATE_LIMIT").text().await?.unwrap_or_else(|| "100".to_string()).parse().unwrap_or(100),
            min_chunk_size: kv.get("MIN_CHUNK_SIZE").text().await?.unwrap_or_else(|| "51200".to_string()).parse().unwrap_or(51200),
            max_chunk_size: kv.get("MAX_CHUNK_SIZE").text().await?.unwrap_or_else(|| "131072".to_string()).parse().unwrap_or(131072).min(131072),
            max_parallel_uploads: kv.get("MAX_PARALLEL_UPLOADS").text().await?.unwrap_or_else(|| "5".to_string()).parse().unwrap_or(5),
            allowed_roles,
            role_permissions,
            path_format: kv.get("PATH_FORMAT").text().await?.unwrap_or_else(|| "{role}/{user_id}/content/{date}/{content_type}/{unique_id}.{extension}".to_string()),
        };

        CONFIG.set(Arc::new(config.clone())).unwrap();
        Ok(config)
    }

    pub fn is_role_allowed(&self, role: &UserRole) -> bool {
        self.allowed_roles.contains(role)
    }

    pub fn has_permission(&self, role: &UserRole, permission: &str) -> bool {
        self.role_permissions.get(role)
            .map(|permissions| permissions.contains(&permission.to_string()))
            .unwrap_or(false)
    }
}