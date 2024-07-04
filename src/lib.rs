use worker::*;

mod config;
mod cors;
mod durable_object;
mod errors;
mod handlers;
mod logging;
mod models;
mod utils;

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    utils::set_panic_hook();

    // Handle CORS preflight requests
    if req.method() == Method::Options {
        return cors::handle_cors_preflight();
    }

    let config = config::Config::load(&env).await?;
    let config = std::sync::Arc::new(config);

    let router = Router::with_data(config);
    let response = router
        .post_async("/v1/uploads/init", handlers::handle_upload_init)
        .post_async("/v1/uploads/:id/chunk", handlers::handle_upload_chunk)
        .post_async("/v1/uploads/:id/complete", handlers::handle_complete_upload)
        .get_async("/v1/uploads/:id", handlers::handle_get_upload_status)
        .delete_async("/v1/uploads/:id", handlers::handle_cancel_upload)
        .get_async("/v1/health", handlers::handle_health_check)
        .run(req, env)
        .await?;

    cors::add_cors_headers(response)
}

#[durable_object]
pub struct UploadTrackerObject {
    upload_tracker: durable_object::UploadTracker,
}

#[durable_object]
impl DurableObject for UploadTrackerObject {
    fn new(state: State, env: Env) -> Self {
        Self {
            upload_tracker: durable_object::UploadTracker::new(state, env),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        self.upload_tracker.fetch(req).await
    }
}