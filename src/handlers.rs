use worker::*;
use std::sync::Arc;
use crate::config::Config;
use crate::errors::AppError;
use crate::utils;
use crate::logging::Logger;
use serde_json::Value;

pub async fn handle_upload_init(mut req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling upload initialization request", None);
    let body: Value = match req.json().await {
        Ok(json) => json,
        Err(e) => {
            logger.error(&format!("Failed to parse JSON: {:?}", e), None);
            return Err(AppError::BadRequest("Invalid JSON data".to_string()).into());
        }
    };

    logger.info("Request body", Some(body.clone()));

    forward_to_durable_object(req, &env, config, logger).await
}

pub async fn handle_upload_chunk(mut req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling chunk upload request", None);
    forward_to_durable_object(req, &env, config, logger).await
}

pub async fn handle_get_progress(req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling progress check request", None);
    forward_to_durable_object(req, &env, config, logger).await
}

pub async fn handle_cancel_upload(req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling upload cancellation request", None);
    forward_to_durable_object(req, &env, config, logger).await
}

pub async fn handle_health_check(_req: Request, _ctx: RouteContext<Arc<Config>>, logger: &Logger) -> Result<Response> {
    logger.info("Handling health check request", None);
    utils::json_response(&serde_json::json!({"status": "healthy"}))
}

async fn forward_to_durable_object(req: Request, env: &Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    let durable = env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(&config.tracker_name)?;
    let stub = id.get_stub()?;

    match stub.fetch_with_request(req).await {
        Ok(resp) => Ok(resp),
        Err(e) => {
            logger.error(&format!("Error forwarding request to Durable Object: {:?}", e), None);
            Err(AppError::Internal("Failed to process request".to_string()).into())
        }
    }
}