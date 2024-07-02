use worker::*;
use std::sync::Arc;
use crate::config::Config;
use crate::errors::AppError;
use crate::utils;
use crate::logging::Logger;
use crate::models::UserRole;
use serde_json::Value;
use wasm_bindgen::JsValue;

pub async fn handle_upload_init(mut req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
    logger.info("Handling upload initialization request", None);

    let body = req.json::<Value>().await.map_err(|e| {
        logger.error(&format!("Failed to parse JSON: {:?}", e), None);
        AppError::BadRequest("Invalid JSON data".to_string())
    })?;

    logger.info("Request body", Some(body.clone()));

    let user_id = body["userId"].as_str().ok_or_else(|| AppError::BadRequest("Missing userId".to_string()))?.to_string();
    let user_role = parse_user_role(&body["userRole"])?;
    let content_type = body["contentType"].as_str().ok_or_else(|| AppError::BadRequest("Missing contentType".to_string()))?.to_string();

    let mut new_body = body.clone();
    new_body["userId"] = serde_json::Value::String(user_id);
    new_body["userRole"] = serde_json::Value::String(format!("{:?}", user_role));
    new_body["contentType"] = serde_json::Value::String(content_type);

    let new_req = create_new_request(req.url()?.as_str(), &new_body)?;

    forward_to_durable_object(new_req, &env, config, logger).await
}

pub async fn handle_upload_chunk(req: Request, env: Env, config: &Arc<Config>, logger: &Logger) -> Result<Response> {
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

fn parse_user_role(value: &Value) -> Result<UserRole> {
    match value.as_str().ok_or_else(|| AppError::BadRequest("Missing userRole".to_string()))? {
        "Creator" => Ok(UserRole::Creator),
        "Member" => Ok(UserRole::Member),
        "Subscriber" => Ok(UserRole::Subscriber),
        _ => Err(AppError::BadRequest("Invalid userRole".to_string()).into()),
    }
}

fn create_new_request(url: &str, body: &Value) -> Result<Request> {
    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    Request::new_with_init(
        url,
        RequestInit::new()
            .with_method(Method::Post)
            .with_headers(headers)
            .with_body(Some(JsValue::from_str(&serde_json::to_string(body)?)))
    )
}