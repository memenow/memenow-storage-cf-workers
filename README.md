# MemeNow Storage - Cloudflare Workers

A high-performance, edge-based file storage service built with Rust and Cloudflare Workers. This service provides robust multipart upload capabilities for large files using R2 storage, D1 database for metadata tracking, and KV storage for configuration management.

## Overview

MemeNow Storage is designed to handle large file uploads efficiently at the edge using industry best practices for distributed systems. The service leverages Cloudflare's global network to provide low-latency uploads from anywhere in the world.

### Key Capabilities

- **Large File Support**: Handle files up to 10GB with multipart uploads
- **Edge Performance**: Global distribution via Cloudflare's edge network
- **Reliable State Management**: ACID-compliant upload tracking with D1 database
- **Scalable Storage**: R2 object storage with automatic replication
- **Role-Based Organization**: Hierarchical file organization by user role
- **Production Ready**: Comprehensive error handling and monitoring

## Architecture

The service follows a modern serverless architecture with clear separation of concerns:

```text
┌─────────────────┐    ┌──────────────────┐    ┌─────────────────┐
│   HTTP Client   │───▶│ Cloudflare Edge  │───▶│  Rust Worker    │
└─────────────────┘    └──────────────────┘    └─────────────────┘
                                                         │
                       ┌─────────────────────────────────┼─────────────────────────────────┐
                       │                                 │                                 │
                       ▼                                 ▼                                 ▼
              ┌─────────────────┐              ┌─────────────────┐              ┌─────────────────┐
              │   D1 Database   │              │   R2 Storage    │              │   KV Storage    │
              │   (Metadata)    │              │    (Files)      │              │ (Configuration) │
              └─────────────────┘              └─────────────────┘              └─────────────────┘
```

### Technology Stack

- **Runtime**: Cloudflare Workers with WebAssembly
- **Language**: Rust (wasm32-unknown-unknown target)
- **Database**: D1 SQL database for upload metadata
- **Storage**: R2 object storage for file data
- **Configuration**: KV storage for service settings
- **Build Tool**: worker-build for WebAssembly compilation

## Features

### Upload Management

- **Multipart Uploads**: Efficient handling of large files with parallel chunk uploads
- **Progress Tracking**: Real-time upload progress with chunk-level granularity
- **Resumable Uploads**: Continue interrupted uploads from last completed chunk
- **State Persistence**: Reliable state management using D1 database transactions

### File Organization

- **Role-Based Paths**: Automatic organization by user role (creator/member/subscriber)
- **Content Categorization**: Smart categorization based on MIME types
- **Date-Based Structure**: Chronological organization for easy browsing
- **Collision Prevention**: Unique file paths with timestamp and UUID components

### Security & Reliability

- **Input Validation**: Comprehensive validation of all request parameters
- **Size Limits**: Configurable file size limits with enforcement
- **Error Recovery**: Graceful handling of network and storage failures
- **CORS Support**: Full cross-origin request support for web applications

## API Reference

### Base URL

```text
https://your-worker.example.workers.dev
```

### Authentication
Currently, the service uses header-based upload session tracking. Future versions will include comprehensive authentication.

### Endpoints

#### Initialize Upload

Creates a new upload session and prepares R2 multipart upload.

```http
POST /api/upload/init
Content-Type: application/json

{
  "file_name": "video.mp4",
  "total_size": 1073741824,
  "user_id": "user123",
  "user_role": "creator",
  "content_type": "video/mp4"
}
```

**Response:**
```json
{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "chunk_size": 99614720,
  "status": "initiated",
  "r2_key": "creator/user123/20240112/video/video.mp4"
}
```

#### Upload Chunk
Uploads a single chunk of the file.

```http
PUT /api/upload/chunk
Content-Type: application/octet-stream
X-Upload-Id: 550e8400-e29b-41d4-a716-446655440000
X-Chunk-Index: 0

[Binary chunk data]
```

**Response:**
```json
{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "chunk_index": 0,
  "etag": "\"d41d8cd98f00b204e9800998ecf8427e\"",
  "status": "in_progress"
}
```

#### Complete Upload
Finalizes the multipart upload and makes the file available.

```http
POST /api/upload/complete
Content-Type: application/json

{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response:**
```json
{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "completed",
  "r2_key": "creator/user123/20240112/video/video.mp4"
}
```

#### Get Upload Status
Retrieves current upload progress and metadata.

```http
GET /api/upload/{upload_id}/status
```

**Response:**
```json
{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "in_progress",
  "total_size": 1073741824,
  "chunks": [0, 1, 2, 3, 4],
  "chunk_size": 99614720,
  "r2_key": "creator/user123/20240112/video/video.mp4",
  "updated_at": "2024-01-12T10:35:00Z"
}
```

#### Cancel Upload
Cancels an ongoing upload and cleans up resources.

```http
POST /api/upload/cancel
Content-Type: application/json

{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

**Response:**
```json
{
  "upload_id": "550e8400-e29b-41d4-a716-446655440000",
  "status": "cancelled"
}
```

## Configuration

The service uses KV storage for configuration with intelligent defaults:

| Setting | Default | Description |
|---------|---------|-------------|
| `database_name` | `UPLOAD_DB` | D1 database binding name |
| `max_file_size` | `10737418240` | Maximum file size (10GB) |
| `chunk_size` | `99614720` | Upload chunk size (95 MiB) |

### Configuration Example
```json
{
  "database_name": "UPLOAD_DB",
  "max_file_size": 10737418240,
  "chunk_size": 99614720
}
```

## File Organization

Files are automatically organized using a hierarchical structure:

```text
{user_role}/{user_id}/{date}/{category}/{filename}
```

### Examples
```text
creator/user123/20240112/video/presentation.mp4
member/user456/20240112/image/profile.jpg
subscriber/user789/20240112/document/report.pdf
```

### Content Categories
- `image/` - Image files (JPEG, PNG, GIF, etc.)
- `video/` - Video files (MP4, AVI, MOV, etc.)
- `audio/` - Audio files (MP3, WAV, AAC, etc.)
- `document/` - Text and document files
- `other/` - All other file types

## Development

### Prerequisites
- Rust 1.82 or later
- Cloudflare Workers CLI (`wrangler`)
- Cloudflare account with Workers Paid plan

### Required Cloudflare Resources

1. **R2 Bucket**: Object storage for files
2. **D1 Database**: SQL database for upload metadata
3. **KV Namespace**: Configuration storage
4. **Workers**: Serverless compute environment

### Setup Instructions

1. **Clone the repository**
   ```bash
   git clone https://github.com/memenow/memenow-storage-cf-workers.git
   cd memenow-storage-cf-workers
   ```

2. **Install dependencies**
   ```bash
   cargo install worker-build
   npm install -g wrangler
   ```

3. **Configure Cloudflare resources**
   ```bash
   # Create R2 bucket
   wrangler r2 bucket create memenow-storage

   # Create D1 database
   wrangler d1 create memenow-uploads
   wrangler d1 execute memenow-uploads --file schema.sql

   # Create KV namespace
   wrangler kv namespace create "STORAGE_CONFIG"
   ```

4. **Configure wrangler.toml**
   ```bash
   cp wrangler.toml.template wrangler.toml
   # Edit wrangler.toml with your resource IDs
   ```

5. **Local development**
   ```bash
   wrangler dev
   ```

6. **Deploy to production**
   ```bash
   wrangler deploy
   ```

### Environment Variables

Create a `.env` file for local development:
```bash
PROD_KV_NAMESPACE_ID=your_prod_kv_id
DEV_KV_NAMESPACE_ID=your_dev_kv_id
PROD_D1_DATABASE_ID=your_prod_d1_id
DEV_D1_DATABASE_ID=your_dev_d1_id
```

## Error Handling

The service provides structured error responses with appropriate HTTP status codes:

### Error Response Format
```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error description",
    "timestamp": "2024-01-12T10:30:00Z"
  }
}
```

### Common Error Codes
- `MISSING_FIELD` (400): Required field missing from request
- `VALIDATION_ERROR` (400): Request validation failed
- `INVALID_FIELD` (400): Invalid field value or format
- `INVALID_CHUNK_INDEX` (400): Chunk index out of range
- `UPLOAD_NOT_FOUND` (404): Upload session not found
- `UPLOAD_COMPLETED` (409): Upload already completed
- `UPLOAD_CANCELLED` (409): Upload was cancelled
- `FILE_TOO_LARGE` (413): File exceeds size limit
- `DATABASE_ERROR` (500): D1 operation failed
- `INTERNAL_ERROR` (500): Internal server error
- `R2_ERROR` (502): R2 storage operation failed

## Monitoring

### Health Check
```http
GET /health
```

Returns service health status and current timestamp.

### Metrics
The service provides built-in observability through:
- Cloudflare Workers Analytics
- D1 database query metrics
- R2 storage operation metrics
- Custom error tracking

## Performance & Design

### Design Choices
- **95 MiB default chunks**: sized to stay under the Cloudflare Workers
  request body cap while keeping multipart overhead small
- **Concurrent chunk uploads**: chunks may be issued in parallel by the client;
  D1 upserts on `(upload_id, chunk_index)` keep state consistent
- **Per-isolate config cache**: `Config` is loaded from KV at most once per
  worker isolate via `OnceLock`, keeping the per-request hot path free of KV
  round-trips
- **D1 indexes**: indexes on `user_id`, `status`, `created_at`, `user_role`
  back the typical query patterns (see `schema.sql`)

## Security

### Current Security Measures
- Input validation and sanitization
- File size limits enforcement
- Path traversal attack prevention
- Structured error responses (no information leakage)
- CORS configuration for web application support

> **Note**: `validate_content_type` is a coarse MIME-prefix allowlist intended to
> catch obvious misuse, not a security boundary. The service trusts the
> client-declared `Content-Type` and never inspects file bytes; deploy behind an
> authenticated gateway and add content scanning if untrusted clients can call
> the API directly.

### Future Enhancements
- JWT-based authentication
- Rate limiting per user/IP
- Virus scanning integration
- Access control lists (ACLs)
- Audit logging

## Documentation

Comprehensive documentation is available:

- [API Documentation](./docs/API.md) - Complete REST API reference
- [Architecture Guide](./docs/ARCHITECTURE.md) - System design and components
- [Deployment Guide](./docs/DEPLOYMENT.md) - Production deployment instructions

## Contributing

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

## License

This project is licensed under the Apache 2.0 License. See [LICENSE](./LICENSE) for details.