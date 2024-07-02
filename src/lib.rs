use worker::*;
use std::sync::Arc;

mod config;
mod errors;
mod models;
mod handlers;
mod durable_object;
mod utils;
mod logging;
mod cors;

use crate::config::Config;
use crate::logging::Logger;
use crate::cors::{handle_cors_preflight, add_cors_headers};

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    utils::set_panic_hook();

    let logger = Logger::new("static-request-id".to_string());
    logger.info("Received request", Some(serde_json::json!({
        "method": req.method().to_string(),
        "path": req.path()
    })));

    if req.method() == Method::Options {
        return handle_cors_preflight();
    }

    let config = Config::load(&env).await?;
    let config = Arc::new(config);

    let router = Router::with_data(config);
    let response = router
        .post_async("/v1/uploads/init", |req, ctx| async move {
            let logger = Logger::new("static-request-id".to_string());
            handlers::handle_upload_init(req, ctx.env, &ctx.data, &logger).await
        })
        .post_async("/v1/uploads/chunk", |req, ctx| async move {
            let logger = Logger::new("static-request-id".to_string());
            handlers::handle_upload_chunk(req, ctx.env, &ctx.data, &logger).await
        })
        .get_async("/v1/uploads/:id", |req, ctx| async move {
            let logger = Logger::new("static-request-id".to_string());
            handlers::handle_get_progress(req, ctx.env, &ctx.data, &logger).await
        })
        .delete_async("/v1/uploads/:id", |req, ctx| async move {
            let logger = Logger::new("static-request-id".to_string());
            handlers::handle_cancel_upload(req, ctx.env, &ctx.data, &logger).await
        })
        .get_async("/v1/health", |req, ctx| async move {
            let logger = Logger::new("static-request-id".to_string());
            handlers::handle_health_check(req, ctx, &logger).await
        })
        .run(req, env)
        .await?;

    Ok(add_cors_headers(response))
}