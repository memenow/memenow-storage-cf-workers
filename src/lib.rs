use worker::*;
use worker::kv::KvStore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::str::FromStr;
use std::sync::Arc;
use chrono::{DateTime, Utc};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct Config {
    durable_object_name: String,
    max_file_size: u64,
    chunk_size: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            durable_object_name: "UPLOAD_TRACKER".to_string(),
            max_file_size: 10_737_418_240,
            chunk_size: 157_286_400,
        }
    }
}

impl Config {
    async fn load(kv: &KvStore) -> Result<Self> {
        match kv.get("config").json().await? {
            Some(config) => Ok(config),
            None => {
                console_log!("Config not found in KV, using default");
                Ok(Self::default())
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
enum UserRole {
    Creator,
    Member,
    Subscriber,
}

impl UserRole {
    fn as_str(&self) -> &'static str {
        match self {
            UserRole::Creator => "creator",
            UserRole::Member => "member",
            UserRole::Subscriber => "subscriber",
        }
    }
}

impl FromStr for UserRole {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "creator" => Ok(UserRole::Creator),
            "member" => Ok(UserRole::Member),
            "subscriber" => Ok(UserRole::Subscriber),
            _ => Err(format!("Invalid user role: {}", s)),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct UploadMetadata {
    upload_id: String,
    file_name: String,
    total_size: u64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    user_role: UserRole,
    content_type: String,
    status: UploadStatus,
    chunks: Vec<u16>,
    r2_key: String,
    user_id: String,
    r2_upload_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
enum UploadStatus {
    Initiated,
    InProgress,
    Completed,
    Cancelled,
}

#[durable_object]
pub struct UploadTracker {
    state: State,
    env: Env,
    config: Config,
}

#[durable_object]
impl DurableObject for UploadTracker {
    fn new(state: State, env: Env) -> Self {
        Self {
            state,
            env,
            config: Config::default(),
        }
    }

    async fn fetch(&mut self, req: Request) -> Result<Response> {
        console_log!("UploadTracker::fetch called");
        console_log!("Request method: {:?}", req.method());
        console_log!("Request path: {:?}", req.url()?.path());

        if self.config == Config::default() {
            console_log!("Loading config from KV");
            let kv = self.env.kv("CONFIG")?;
            self.config = Config::load(&kv).await?;
        }

        let url = req.url()?;
        let path = url.path();
        let method = req.method();

        match (method, path) {
            (Method::Post, "/v1/uploads/init") => {
                console_log!("Handling initiate request");
                self.handle_initiate(req).await
            },
            (Method::Post, p) if p.contains("/chunk") => self.handle_upload_chunk(req).await,
            (Method::Post, p) if p.contains("/complete") => self.handle_complete(req).await,
            (Method::Get, p) if p.starts_with("/v1/uploads/") => self.handle_status(req).await,
            (Method::Delete, p) if p.starts_with("/v1/uploads/") => self.handle_cancel(req).await,
            _ => {
                console_log!("No matching route found");
                Response::error("Not Found", 404)
            }
        }
    }
}

impl UploadTracker {
    async fn handle_initiate(&mut self, mut req: Request) -> Result<Response> {
        console_log!("Handling initiate request");
        let body: serde_json::Value = req.json().await?;
        let upload_id = generate_unique_identifier();
        let metadata = UploadMetadata {
            upload_id: upload_id.clone(),
            file_name: body["fileName"].as_str().ok_or("Missing fileName")?.to_string(),
            total_size: body["totalSize"].as_u64().ok_or("Invalid totalSize")?,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            user_role: UserRole::from_str(body["userRole"].as_str().ok_or("Missing userRole")?).map_err(|e| Error::from(e))?,
            content_type: body["contentType"].as_str().ok_or("Missing contentType")?.to_string(),
            status: UploadStatus::Initiated,
            chunks: vec![],
            r2_key: generate_r2_key(&body),
            user_id: body["userId"].as_str().ok_or("Missing userId")?.to_string(),
            r2_upload_id: String::new(),
        };

        if metadata.total_size > self.config.max_file_size {
            return Response::error("File size exceeds maximum allowed", 400);
        }

        self.state.storage().put(&upload_id, &metadata).await?;

        Response::from_json(&json!({
            "message": "Multipart upload initiated",
            "uploadId": upload_id,
            "r2Key": metadata.r2_key,
        }))
    }

    async fn handle_upload_chunk(&mut self, mut req: Request) -> Result<Response> {
        console_log!("Handling upload chunk request");
        let upload_id = req.headers().get("X-Upload-Id")?
            .ok_or_else(|| Error::from("Missing X-Upload-Id header"))?;
        let chunk_index: u16 = req.headers().get("X-Chunk-Index")?
            .ok_or_else(|| Error::from("Missing X-Chunk-Index header"))?
            .parse().map_err(|e| Error::from(format!("Invalid X-Chunk-Index: {}", e)))?;

        let mut metadata = match self.state.storage().get::<UploadMetadata>(&upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error(format!("Error retrieving upload: {:?}", e), 500);
            }
        };

        let chunk_data = req.bytes().await?;
        let r2 = self.env.bucket("BUCKET")?;

        let multipart_upload = if chunk_index == 1 {
            let new_upload = r2.create_multipart_upload(&metadata.r2_key).execute().await?;
            metadata.r2_upload_id = new_upload.upload_id().await.to_string();
            new_upload
        } else {
            r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id)?
        };

        let part = multipart_upload.upload_part(chunk_index, chunk_data).await?;

        metadata.chunks.push(chunk_index);
        metadata.status = UploadStatus::InProgress;
        metadata.updated_at = Utc::now();

        self.state.storage().put(&upload_id, &metadata).await?;

        Response::from_json(&json!({
            "message": "Chunk uploaded successfully",
            "chunkIndex": chunk_index,
            "etag": part.etag(),
            "uploadId": metadata.upload_id,
            "r2UploadId": metadata.r2_upload_id,
        }))
    }

    async fn handle_complete(&mut self, mut req: Request) -> Result<Response> {
        console_log!("Handling complete upload request");
        let body: serde_json::Value = req.json().await?;

        console_log!("Request body: {:?}", body);

        let upload_id = body["uploadId"].as_str().ok_or_else(|| Error::from("Missing uploadId"))?;

        let mut metadata = match self.state.storage().get::<UploadMetadata>(upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error(format!("Error retrieving upload: {:?}", e), 500);
            }
        };

        let parts = body["parts"].as_array().ok_or_else(|| Error::from("Missing or invalid 'parts'"))?;

        if parts.is_empty() {
            return Response::error("No parts provided", 400);
        }

        let r2 = self.env.bucket("BUCKET")?;
        let complete_parts: Vec<worker::UploadedPart> = parts.iter().map(|part| {
            let etag = part["etag"].as_str().unwrap_or("");
            let part_number = part["partNumber"].as_u64().unwrap_or(0) as u16;
            worker::UploadedPart::new(part_number, etag.to_string())
        }).collect();

        let multipart_upload = r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id)?;

        match multipart_upload.complete(complete_parts).await {
            Ok(_) => {
                metadata.status = UploadStatus::Completed;
                metadata.updated_at = Utc::now();
                self.state.storage().put(upload_id, &metadata).await?;

                Response::from_json(&json!({
                    "message": "Multipart upload completed successfully",
                    "uploadId": upload_id,
                }))
            }
            Err(e) => {
                console_error!("Failed to complete multipart upload: {:?}", e);
                Response::error("Failed to complete multipart upload", 500)
            }
        }
    }

    async fn handle_cancel(&mut self, req: Request) -> Result<Response> {
        console_log!("Handling cancel upload request");
        let upload_id = req.headers().get("X-Upload-Id")?
            .ok_or_else(|| Error::from("Missing X-Upload-Id header"))?;

        let mut metadata = match self.state.storage().get::<UploadMetadata>(&upload_id).await {
            Ok(data) => data,
            Err(e) => {
                if e.to_string().contains("not found") {
                    return Response::error("Upload not found", 404);
                }
                return Response::error(format!("Error retrieving upload: {:?}", e), 500);
            }
        };

        let r2 = self.env.bucket("BUCKET")?;
        r2.resume_multipart_upload(&metadata.r2_key, &metadata.r2_upload_id)?
            .abort()
            .await?;

        metadata.status = UploadStatus::Cancelled;
        metadata.updated_at = Utc::now();
        self.state.storage().put(&upload_id, &metadata).await?;

        Response::from_json(&json!({"message": "Upload cancelled successfully"}))
    }

    async fn handle_status(&self, req: Request) -> Result<Response> {
        console_log!("Handling get upload status request");
        let upload_id = req.headers().get("X-Upload-Id")?
            .ok_or_else(|| Error::from("Missing X-Upload-Id header"))?;
        let metadata: UploadMetadata = self.state.storage().get(&upload_id).await?;

        Response::from_json(&json!({
            "uploadId": metadata.upload_id,
            "fileName": metadata.file_name,
            "totalSize": metadata.total_size,
            "uploadedChunks": metadata.chunks,
            "status": format!("{:?}", metadata.status),
        }))
    }

}

fn generate_r2_key(body: &serde_json::Value) -> String {
    let user_role = UserRole::from_str(body["userRole"].as_str().unwrap()).unwrap();
    let user_id = body["userId"].as_str().unwrap();
    let content_type = body["contentType"].as_str().unwrap();
    let file_name = body["fileName"].as_str().unwrap();

    let date = Utc::now().format("%Y%m%d").to_string();
    let content_category = content_type.split('/').next().unwrap_or("unknown");
    format!("{}/{}/{}/{}/{}", user_role.as_str(), user_id, date, content_category, file_name)
}

fn generate_unique_identifier() -> String {
    use rand::Rng;
    let random_part: u64 = rand::thread_rng().gen();
    format!("{:x}", random_part)
}

fn cors_headers() -> Headers {
    let mut headers = Headers::new();
    headers.set("Access-Control-Allow-Origin", "*").unwrap();
    headers.set("Access-Control-Allow-Methods", "GET, POST, PUT, DELETE, OPTIONS").unwrap();
    headers.set("Access-Control-Allow-Headers", "Content-Type, X-Upload-Id, X-Chunk-Index").unwrap();
    headers
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    console_log!("Received request: {:?} {}", req.method(), req.url()?);

    if req.method() == Method::Options {
        return Ok(Response::empty()?.with_headers(cors_headers()));
    }

    let kv = env.kv("CONFIG")?;
    let config = Config::load(&kv).await?;
    let config = Arc::new(config);

    let namespace = env.durable_object(config.durable_object_name.as_str())?;
    let id = namespace.id_from_name("UPLOAD_TRACKER")?;
    let stub = id.get_stub()?;

    let do_response = stub.fetch_with_request(req).await?;

    Ok(do_response.with_headers(cors_headers()))
}