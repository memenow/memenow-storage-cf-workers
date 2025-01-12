# memenow-storage-cf-workers

A high-performance, edge-based file storage service implemented with Cloudflare Workers and Rust. The service supports chunked uploads for large files, leveraging Cloudflare's R2 storage and Durable Objects for state management.

## Features

* Chunked file upload support for handling large files
* Upload state management using Durable Objects
* Multipart upload capabilities with R2 storage
* Role-based file organization
* CORS support
* Configurable via KV storage

## Configuration

Configuration is managed through Cloudflare KV storage with the following default values:

```rust
Config {
    durable_object_name: "UPLOAD_TRACKER",
    max_file_size: 10_737_418_240,  // 10 GB
    chunk_size: 157_286_400,        // 150 MB
}
```

## API Endpoints

### Initialize Upload
```
POST /v1/uploads/init
Content-Type: application/json

{
    "fileName": string,
    "totalSize": number,
    "userRole": "creator" | "member" | "subscriber",
    "contentType": string,
    "userId": string
}
```

### Upload Chunk
```
POST /v1/uploads/{uploadId}/chunk
Headers:
    X-Upload-Id: string
    X-Chunk-Index: number
Body: binary
```

### Complete Upload
```
POST /v1/uploads/{uploadId}/complete
Content-Type: application/json

{
    "uploadId": string,
    "parts": [
        {
            "etag": string,
            "partNumber": number
        }
    ]
}
```

### Get Upload Status
```
GET /v1/uploads/{uploadId}
Headers:
    X-Upload-Id: string
```

### Cancel Upload
```
DELETE /v1/uploads/{uploadId}
Headers:
    X-Upload-Id: string
```

## File Organization

Files are organized in R2 storage using the following path structure:
```
{userRole}/{userId}/{date}/{contentCategory}/{fileName}
```

Example:
```
creator/user123/20240112/image/profile.jpg
```

## Error Handling

The service implements comprehensive error handling for:
* File size limits
* Missing or invalid headers
* Upload state validation
* R2 storage operations
* Invalid user roles

## Development

### Prerequisites
* Rust
* wrangler CLI
* Cloudflare account with Workers, R2, and KV access

### Required Cloudflare Resources
* R2 bucket named "BUCKET"
* KV namespace named "CONFIG"
* Durable Object named "UPLOAD_TRACKER"

### Local Development
1. Clone the repository
2. Configure wrangler.toml with your Cloudflare account details
3. Run `wrangler dev` for local development

### Deployment
```bash
wrangler publish
```

## Security Considerations

* File size limits are enforced
* CORS headers are configured for cross-origin requests
* Role-based file organization
* Unique upload identifiers

## License

This project is licensed under the Apache 2.0 License. For more details, please refer to the [LICENSE](./LICENSE) file.