use worker::*;
use std::sync::Arc;
use crate::config::Config;
use crate::errors::AppError;
use crate::{cors, utils};
use serde_json::json;
use crate::logging::Logger;

pub async fn handle_upload_init(mut req: Request, ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    let config = &ctx.data;
    let env = &ctx.env;
    let logger = Logger::new(utils::generate_request_id());

    logger.info("Handling upload initialization", None);

    let body: serde_json::Value = req.json().await?;
    let file_name = body["fileName"].as_str().ok_or(AppError::BadRequest("Missing fileName".into()))?;
    let total_size: u64 = body["totalSize"].as_u64().ok_or(AppError::BadRequest("Invalid totalSize".into()))?;
    let user_role = body["userRole"].as_str().ok_or(AppError::BadRequest("Missing userRole".into()))?;
    let content_type = body["contentType"].as_str().ok_or(AppError::BadRequest("Missing contentType".into()))?;

    if total_size > config.max_file_size {
        logger.warn("File size exceeds maximum allowed", Some(json!({
            "size": total_size,
            "max_size": config.max_file_size
        })));
        return Err(AppError::FileTooLarge("File size exceeds maximum allowed".into()).into());
    }

    let upload_id = utils::generate_unique_identifier();

    let durable = env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(&upload_id)?;
    let stub = id.get_stub()?;

    let init_data = json!({
        "action": "initiate",
        "uploadId": upload_id,
        "fileName": file_name,
        "totalSize": total_size,
        "userRole": user_role,
        "contentType": content_type,
    });

    logger.info("Initiating upload", Some(json!({ "uploadId": upload_id })));

    let serialized_data = serde_json::to_string(&init_data).map_err(|e| {
        logger.error("Failed to serialize init_data", Some(json!({ "error": format!("{:?}", e) })));
        AppError::Internal("Failed to serialize init_data".into())
    })?;

    let response = stub.fetch_with_str(&serialized_data).await.map_err(|e| {
        logger.error("Failed to fetch durable object", Some(json!({ "error": format!("{:?}", e) })));
        worker::Error::RustError(format!("Failed to fetch durable object: {:?}", e))
    })?;

    cors::add_cors_headers(response)
}

pub async fn handle_upload_chunk(mut req: Request, ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    let config = &ctx.data;
    let upload_id = ctx.param("id").ok_or(AppError::BadRequest("Missing uploadId".into()))?;
    let chunk_index: u16 = req.headers()
        .get("X-Chunk-Index")?
        .ok_or(AppError::BadRequest("Missing X-Chunk-Index header".into()))?
        .parse()
        .map_err(|_| AppError::BadRequest("Invalid X-Chunk-Index header".into()))?;

    let durable = ctx.env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(upload_id)?;
    let stub = id.get_stub()?;

    let chunk_data = req.bytes().await?;
    let etag = utils::calculate_etag(&chunk_data);

    let chunk_info = json!({
        "action": "uploadChunk",
        "uploadId": upload_id,
        "chunkIndex": chunk_index,
        "etag": etag,
    });

    let mut headers = Headers::new();
    headers.set("Content-Type", "application/json")?;

    let mut req_init = RequestInit::new();
    req_init.with_method(Method::Post)
        .with_body(Some(serde_json::to_string(&chunk_info)?.into()))
        .with_headers(headers);

    let request = Request::new_with_init("", &req_init)?;
    let response = stub.fetch_with_request(request).await.map_err(|e| {
        worker::Error::RustError(format!("Failed to fetch with request: {:?}", e))
    })?;

    cors::add_cors_headers(response)
}

pub async fn handle_complete_upload(mut req: Request, ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    let config = &ctx.data;
    let upload_id = ctx.param("id").ok_or(AppError::BadRequest("Missing uploadId".into()))?;
    let body: serde_json::Value = req.json().await?;

    let durable = ctx.env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(upload_id)?;
    let stub = id.get_stub()?;

    let complete_data = json!({
        "action": "complete",
        "uploadId": upload_id,
        "parts": body["parts"],
    });

    let serialized_data = serde_json::to_string(&complete_data).map_err(|e| {
        worker::Error::RustError(format!("Failed to serialize complete_data: {:?}", e))
    })?;

    let response = stub.fetch_with_str(&serialized_data).await.map_err(|e| {
        worker::Error::RustError(format!("Failed to fetch with complete_data: {:?}", e))
    })?;

    cors::add_cors_headers(response)
}

pub async fn handle_get_upload_status(_req: Request, ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    let config = &ctx.data;
    let upload_id = ctx.param("id").ok_or(AppError::BadRequest("Missing uploadId".into()))?;

    let durable = ctx.env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(upload_id)?;
    let stub = id.get_stub()?;

    let status_data = json!({
        "action": "getStatus",
        "uploadId": upload_id,
    });

    let serialized_data = serde_json::to_string(&status_data).map_err(|e| {
        worker::Error::RustError(format!("Failed to serialize status_data: {:?}", e))
    })?;

    let response = stub.fetch_with_str(&serialized_data).await.map_err(|e| {
        worker::Error::RustError(format!("Failed to fetch with status_data: {:?}", e))
    })?;

    cors::add_cors_headers(response)
}

pub async fn handle_cancel_upload(_req: Request, ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    let config = &ctx.data;
    let upload_id = ctx.param("id").ok_or(AppError::BadRequest("Missing uploadId".into()))?;

    let durable = ctx.env.durable_object(&config.durable_object_name)?;
    let id = durable.id_from_name(upload_id)?;
    let stub = id.get_stub()?;

    let cancel_data = json!({
        "action": "cancel",
        "uploadId": upload_id,
    });

    let serialized_data = serde_json::to_string(&cancel_data).map_err(|e| {
        worker::Error::RustError(format!("Failed to serialize cancel_data: {:?}", e))
    })?;

    let response = stub.fetch_with_str(&serialized_data).await.map_err(|e| {
        worker::Error::RustError(format!("Failed to fetch with cancel_data: {:?}", e))
    })?;

    cors::add_cors_headers(response)
}

pub async fn handle_health_check(_req: Request, _ctx: RouteContext<Arc<Config>>) -> Result<Response> {
    utils::json_response(&json!({"status": "healthy"}))
}
