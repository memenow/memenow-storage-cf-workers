use worker::*;
use std::sync::Arc;
use once_cell::sync::OnceCell;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Config {
    pub durable_object_name: String,
    pub tracker_name: String,
    pub bucket_name: String,
    pub max_file_size: usize,
    pub rate_limit: u32,
}

static CONFIG: OnceCell<Arc<Config>> = OnceCell::new();

impl Config {
    pub async fn load(env: &Env) -> Result<Config> {
        if let Some(config) = CONFIG.get() {
            return Ok(config.as_ref().clone());
        }

        let kv = env.kv("CONFIG")?;
        let config = Config {
            durable_object_name: kv.get("DURABLE_OBJECT_NAME").text().await?.unwrap_or_else(|| "UPLOAD_TRACKER".to_string()),
            tracker_name: kv.get("TRACKER_NAME").text().await?.unwrap_or_else(|| "tracker".to_string()),
            bucket_name: kv.get("BUCKET_NAME").text().await?.unwrap_or_else(|| "BUCKET".to_string()),
            max_file_size: kv.get("MAX_FILE_SIZE").text().await?.unwrap_or_else(|| "100000000".to_string()).parse().unwrap_or(100_000_000),
            rate_limit: kv.get("RATE_LIMIT").text().await?.unwrap_or_else(|| "100".to_string()).parse().unwrap_or(100),
        };

        CONFIG.set(Arc::new(config.clone())).unwrap();
        Ok(config)
    }
}