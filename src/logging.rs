use worker::*;
use serde_json::json;
use chrono::Utc;

#[derive(Clone)]
pub struct Logger {
    request_id: String,
}

impl Logger {
    pub fn new(request_id: String) -> Self {
        Self { request_id }
    }

    pub fn info(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("INFO", message, data);
    }

    pub fn warn(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("WARN", message, data);
    }

    pub fn error(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("ERROR", message, data);
    }

    fn log(&self, level: &str, message: &str, data: Option<serde_json::Value>) {
        let timestamp = Utc::now().to_rfc3339();
        let log_data = json!({
            "timestamp": timestamp,
            "level": level,
            "request_id": self.request_id,
            "message": message,
            "data": data
        });

        match level {
            "INFO" => console_log!("{}", log_data.to_string()),
            "WARN" => console_warn!("{}", log_data.to_string()),
            "ERROR" => console_error!("{}", log_data.to_string()),
            _ => console_log!("{}", log_data.to_string()),
        }
    }
}
