use worker::*;
use serde_json::json;
use chrono::Utc;

/// Logger struct for handling structured logging
pub struct Logger {
    request_id: String,
}

impl Logger {
    /// Create a new Logger instance
    ///
    /// # Arguments
    ///
    /// * `request_id` - A unique identifier for the current request
    pub fn new(request_id: String) -> Self {
        Self { request_id }
    }

    /// Log an info message
    ///
    /// # Arguments
    ///
    /// * `message` - The log message
    /// * `data` - Optional additional data to include in the log
    pub fn info(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("INFO", message, data);
    }

    /// Log a warning message
    ///
    /// # Arguments
    ///
    /// * `message` - The log message
    /// * `data` - Optional additional data to include in the log
    pub fn warn(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("WARN", message, data);
    }

    /// Log an error message
    ///
    /// # Arguments
    ///
    /// * `message` - The log message
    /// * `data` - Optional additional data to include in the log
    pub fn error(&self, message: &str, data: Option<serde_json::Value>) {
        self.log("ERROR", message, data);
    }

    /// Internal method to handle log creation and output
    ///
    /// # Arguments
    ///
    /// * `level` - The log level (INFO, WARN, ERROR)
    /// * `message` - The log message
    /// * `data` - Optional additional data to include in the log
    fn log(&self, level: &str, message: &str, data: Option<serde_json::Value>) {
        let timestamp = Utc::now().to_rfc3339();
        let log_data = json!({
            "timestamp": timestamp,
            "level": level,
            "request_id": self.request_id,
            "message": message,
            "data": data
        });

        // Use console_log for INFO, console_warn for WARN, and console_error for ERROR
        match level {
            "INFO" => console_log!("{}", log_data.to_string()),
            "WARN" => console_warn!("{}", log_data.to_string()),
            "ERROR" => console_error!("{}", log_data.to_string()),
            _ => console_log!("{}", log_data.to_string()),
        }
    }
}

/// Macro to create a JSON object for additional log data
///
/// Usage: log_data!({ "key1" = "value1", "key2" = 42 })
#[macro_export]
macro_rules! log_data {
    ($($key:expr => $value:expr),*) => {
        Some(serde_json::json!({ $($key: $value),* }))
    };
}