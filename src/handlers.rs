use worker::*;
use std::sync::Arc;
use crate::config::Config;
use crate::errors::AppError;
use crate::utils;
use crate::logging::Logger;

pub async fn handle_upload(req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling upload request", None);
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
            logger.error(&format!("Error forwarding request to Durable Object: {}", e), None);
            Err(AppError::Internal("Failed to process request".to_string()).into())
        }
    }
}