# API Documentation

## Overview

The MemeNow Storage API provides a REST interface for managing large file uploads using chunked multipart uploads. The API is built on Cloudflare Workers and provides global edge performance with strong consistency guarantees through D1 database integration.

## Base URL

```text
https://your-worker-name.your-subdomain.workers.dev
```

## Authentication

Currently, the API uses user identification via the `user_id` parameter in requests. Future versions will include JWT-based authentication with role-based access control.

## Architecture

The service uses a modern serverless architecture:

- **Cloudflare Workers**: Edge compute for request processing
- **D1 Database**: SQL database for upload metadata and state management
- **R2 Storage**: Object storage for file data
- **KV Storage**: Configuration and caching

This architecture provides ACID compliance, complex query capabilities, and improved scalability compared to previous implementations.

## Rate Limiting

Rate limiting should be implemented at the Cloudflare dashboard level or via custom middleware for production deployments. Consider implementing per-user rate limits based on user roles.

## Error Handling

All API responses follow a consistent error format with appropriate HTTP status codes and structured JSON error messages.

### Error Response Format

```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable error description",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

### Common Error Codes

| Code | Status | Description |
|------|--------|-------------|
| `MISSING_FIELD` | 400 | Required field missing from request |
| `INVALID_FIELD` | 400 | Field contains invalid value |
| `FILE_TOO_LARGE` | 413 | File exceeds maximum size limit |
| `UPLOAD_NOT_FOUND` | 404 | Upload ID not found |
| `UPLOAD_COMPLETED` | 409 | Upload already completed |
| `UPLOAD_CANCELLED` | 409 | Upload was cancelled |
| `DATABASE_ERROR` | 502 | Database operation failed |
| `R2_ERROR` | 502 | R2 storage operation failed |
| `RATE_LIMIT_EXCEEDED` | 429 | Too many requests |

## API Endpoints

### Health Check

Check service health and status.

```http
GET /health
```

#### Health Check Response

```json
{
  "status": "healthy",
  "service": "memenow-storage-cf-workers",
  "timestamp": "2024-01-15T10:30:00Z"
}
```

**Status Codes:**
- `200` - Service is healthy

---

### Initialize Upload

Create a new upload session for a file.

```http
POST /api/upload/init
Content-Type: application/json
```

#### Initialize Upload Request Body

```json
{
  "file_name": "example.mp4",
  "total_size": 524288000,
  "user_role": "creator",
  "content_type": "video/mp4",
  "user_id": "user_12345"
}
```

##### Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_name` | string | Yes | Original filename |
| `total_size` | number | Yes | Total file size in bytes |
| `user_role` | string | Yes | User role: `creator`, `member`, or `subscriber` |
| `content_type` | string | Yes | MIME type of the file |
| `user_id` | string | Yes | Unique user identifier |

#### Initialize Upload Response

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000",
  "chunk_size": 157286400,
  "status": "initiated"
}
```

##### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `upload_id` | string | Unique upload session identifier |
| `chunk_size` | number | Recommended chunk size in bytes |
| `status` | string | Current upload status |

**Status Codes:**
- `200` - Upload initialized successfully
- `400` - Invalid request parameters
- `413` - File size exceeds maximum allowed

---

### Upload Chunk

Upload a chunk of the file.

```http
PUT /api/upload/chunk
Content-Type: application/octet-stream
X-Upload-Id: {uploadId}
X-Chunk-Index: {chunkNumber}
```

#### Upload Chunk Headers

| Header | Type | Required | Description |
|--------|------|----------|-------------|
| `X-Upload-Id` | string | Yes | Upload session identifier |
| `X-Chunk-Index` | number | Yes | Chunk number (starting from 0) |
| `Content-Type` | string | Yes | Must be `application/octet-stream` |

#### Upload Chunk Request Body

Binary data representing the file chunk.

#### Upload Chunk Response

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000",
  "chunk_index": 0,
  "status": "uploaded"
}
```

##### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `upload_id` | string | Upload session identifier |
| `chunk_index` | number | Index of the uploaded chunk |
| `status` | string | Status of the chunk upload |

**Status Codes:**
- `200` - Chunk uploaded successfully
- `400` - Invalid headers or chunk data
- `404` - Upload session not found

---

### Complete Upload

Complete the multipart upload.

```http
POST /api/upload/complete
Content-Type: application/json
```

#### Complete Upload Request Body

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000"
}
```

##### Complete Upload Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `upload_id` | string | Yes | Upload session identifier |

#### Complete Upload Response

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000",
  "r2_key": "creator/user_12345/20240105/video/example.mp4",
  "status": "completed"
}
```

##### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `upload_id` | string | Upload session identifier |
| `r2_key` | string | Final storage path in R2 |
| `status` | string | Final upload status |

**Status Codes:**
- `200` - Upload completed successfully
- `400` - Invalid request or incomplete upload
- `404` - Upload session not found

---

### Get Upload Status

Retrieve the current status of an upload session.

```http
GET /api/upload/{upload_id}/status
```

#### Get Upload Status Response

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000",
  "file_name": "example.mp4",
  "total_size": 524288000,
  "user_id": "user_12345",
  "user_role": "creator",
  "content_type": "video/mp4",
  "status": "in_progress",
  "chunks_uploaded": 5,
  "created_at": "2024-01-05T10:30:00Z",
  "updated_at": "2024-01-05T10:35:00Z"
}
```

##### Response Fields

| Field | Type | Description |
|-------|------|-------------|
| `upload_id` | string | Upload session identifier |
| `file_name` | string | Original filename |
| `total_size` | number | Total file size in bytes |
| `user_id` | string | User identifier |
| `user_role` | string | User role |
| `content_type` | string | MIME type |
| `status` | string | Current upload status |
| `chunks_uploaded` | number | Number of chunks uploaded |
| `created_at` | string | Upload creation timestamp (ISO 8601) |
| `updated_at` | string | Last update timestamp (ISO 8601) |

##### Status Values

- `initiated` - Upload session created but no chunks uploaded
- `in_progress` - Some chunks have been uploaded
- `completed` - All chunks uploaded and file assembled
- `cancelled` - Upload was cancelled

**Status Codes:**
- `200` - Status retrieved successfully
- `404` - Upload session not found

---

### Cancel Upload

Cancel an active upload session and clean up resources.

```http
POST /api/upload/cancel
Content-Type: application/json
```

#### Cancel Upload Request Body

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000"
}
```

#### Cancel Upload Response

```json
{
  "upload_id": "1704447000000-550e8400-e29b-41d4-a716-446655440000",
  "status": "cancelled"
}
```

**Status Codes:**
- `200` - Upload cancelled successfully
- `404` - Upload session not found

## File Organization

Files are organized in R2 storage using a structured path format that facilitates browsing and management:

```text
{user_role}/{user_id}/{date}/{content_category}/{file_name}
```

### Path Components

- **user_role**: `creator`, `member`, or `subscriber`
- **user_id**: Unique user identifier
- **date**: Upload date in `YYYYMMDD` format
- **content_category**: Determined by content type:
  - `image` - Image files (image/*)
  - `video` - Video files (video/*)
  - `audio` - Audio files (audio/*)
  - `document` - Text and JSON files
  - `other` - Other file types
- **file_name**: Sanitized original filename

### Example Paths

```text
creator/user123/20240115/image/profile.jpg
member/user456/20240115/video/presentation.mp4
subscriber/user789/20240115/document/report.pdf
```

This structure enables:
- Easy browsing by user and date
- Content type filtering
- Role-based access control (future enhancement)
- Scalable storage organization

## Database Schema

The service uses D1 database with the following schema:

### uploads Table

| Column | Type | Description |
|--------|------|-------------|
| upload_id | TEXT PRIMARY KEY | Unique upload identifier |
| file_name | TEXT NOT NULL | Original filename |
| total_size | INTEGER NOT NULL | Total file size in bytes |
| content_type | TEXT NOT NULL | MIME type |
| user_id | TEXT NOT NULL | User identifier |
| user_role | TEXT NOT NULL | User role (creator/member/subscriber) |
| r2_key | TEXT NOT NULL | R2 storage path |
| r2_upload_id | TEXT NOT NULL | R2 multipart upload ID |
| status | TEXT NOT NULL | Upload status |
| created_at | TEXT NOT NULL | Creation timestamp (ISO 8601) |
| updated_at | TEXT NOT NULL | Last update timestamp (ISO 8601) |

### upload_chunks Table

| Column | Type | Description |
|--------|------|-------------|
| upload_id | TEXT | Upload identifier (foreign key) |
| chunk_index | INTEGER | Chunk index number |
| chunk_size | INTEGER NOT NULL | Chunk size in bytes |
| etag | TEXT | R2 ETag for the chunk |
| uploaded_at | TEXT NOT NULL | Upload timestamp (ISO 8601) |

## Usage Examples

### JavaScript SDK Example

```javascript
class MemeNowStorageClient {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
  }

  async uploadFile(file, userId, userRole) {
    // Initialize upload
    const initResponse = await fetch(`${this.baseUrl}/api/upload/init`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        file_name: file.name,
        total_size: file.size,
        user_role: userRole,
        content_type: file.type,
        user_id: userId
      })
    });

    const { upload_id, chunk_size } = await initResponse.json();

    // Upload chunks
    const chunks = Math.ceil(file.size / chunk_size);
    
    for (let i = 0; i < chunks; i++) {
      const start = i * chunk_size;
      const end = Math.min(start + chunk_size, file.size);
      const chunk = file.slice(start, end);
      
      const chunkResponse = await fetch(`${this.baseUrl}/api/upload/chunk`, {
        method: 'PUT',
        headers: {
          'Content-Type': 'application/octet-stream',
          'X-Upload-Id': upload_id,
          'X-Chunk-Index': i.toString()
        },
        body: chunk
      });
      
      if (!chunkResponse.ok) {
        throw new Error(`Chunk upload failed: ${chunkResponse.status}`);
      }
    }

    // Complete upload
    const completeResponse = await fetch(`${this.baseUrl}/api/upload/complete`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ upload_id })
    });

    return await completeResponse.json();
  }

  async getUploadStatus(uploadId) {
    const response = await fetch(`${this.baseUrl}/api/upload/${uploadId}/status`);
    return await response.json();
  }

  async cancelUpload(uploadId) {
    const response = await fetch(`${this.baseUrl}/api/upload/cancel`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ upload_id: uploadId })
    });
    return await response.json();
  }
}

// Usage
const client = new MemeNowStorageClient('https://your-worker.workers.dev');
const result = await client.uploadFile(fileInput.files[0], 'user123', 'creator');
```

### Python SDK Example

```python
import requests
import math
import os

class MemeNowStorageClient:
    def __init__(self, base_url):
        self.base_url = base_url
    
    def upload_file(self, file_path, user_id, user_role):
        with open(file_path, 'rb') as f:
            file_data = f.read()
            file_size = len(file_data)
            file_name = os.path.basename(file_path)
        
        # Initialize upload
        init_response = requests.post(
            f"{self.base_url}/api/upload/init",
            json={
                "file_name": file_name,
                "total_size": file_size,
                "user_role": user_role,
                "content_type": "application/octet-stream",
                "user_id": user_id
            }
        )
        init_response.raise_for_status()
        
        upload_data = init_response.json()
        upload_id = upload_data["upload_id"]
        chunk_size = upload_data["chunk_size"]
        
        # Upload chunks
        chunks = math.ceil(file_size / chunk_size)
        
        for i in range(chunks):
            start = i * chunk_size
            end = min(start + chunk_size, file_size)
            chunk_data = file_data[start:end]
            
            chunk_response = requests.put(
                f"{self.base_url}/api/upload/chunk",
                data=chunk_data,
                headers={
                    "Content-Type": "application/octet-stream",
                    "X-Upload-Id": upload_id,
                    "X-Chunk-Index": str(i)
                }
            )
            chunk_response.raise_for_status()
        
        # Complete upload
        complete_response = requests.post(
            f"{self.base_url}/api/upload/complete",
            json={"upload_id": upload_id}
        )
        complete_response.raise_for_status()
        
        return complete_response.json()
    
    def get_upload_status(self, upload_id):
        response = requests.get(f"{self.base_url}/api/upload/{upload_id}/status")
        response.raise_for_status()
        return response.json()
    
    def cancel_upload(self, upload_id):
        response = requests.post(
            f"{self.base_url}/api/upload/cancel",
            json={"upload_id": upload_id}
        )
        response.raise_for_status()
        return response.json()

# Usage
client = MemeNowStorageClient("https://your-worker.workers.dev")
result = client.upload_file("/path/to/file.mp4", "user123", "creator")
```

## Configuration

### Cloudflare Resources

Configure the following resources in your Cloudflare account:

| Resource | Binding Name | Description |
|----------|--------------|-------------|
| R2 Bucket | `STORAGE_BUCKET` | Object storage for files |
| D1 Database | `UPLOAD_DB` | SQL database for metadata |
| KV Namespace | `STORAGE_CONFIG` | Configuration storage |

### KV Configuration

Store configuration in KV under the key `config`:

```json
{
  "database_name": "UPLOAD_DB",
  "max_file_size": 10737418240,
  "chunk_size": 157286400
}
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `database_name` | string | "UPLOAD_DB" | D1 database binding name |
| `max_file_size` | number | 10737418240 | Maximum file size in bytes (10GB) |
| `chunk_size` | number | 157286400 | Recommended chunk size in bytes (150MB) |

### Environment Setup

1. Create D1 database and apply schema:
   ```bash
   wrangler d1 create memenow-uploads
   wrangler d1 execute memenow-uploads --file schema.sql
   ```

2. Create R2 bucket:
   ```bash
   wrangler r2 bucket create memenow-storage
   ```

3. Create KV namespace:
   ```bash
   wrangler kv:namespace create "STORAGE_CONFIG"
   ```

## Testing

### Health Check Test

```bash
curl -X GET https://your-worker.workers.dev/health
```

### Upload Flow Test

```bash
# 1. Initialize upload
UPLOAD_RESPONSE=$(curl -X POST https://your-worker.workers.dev/api/upload/init \
  -H "Content-Type: application/json" \
  -d '{
    "file_name": "test.txt",
    "total_size": 13,
    "user_role": "creator",
    "content_type": "text/plain",
    "user_id": "test_user"
  }')

UPLOAD_ID=$(echo $UPLOAD_RESPONSE | jq -r '.upload_id')

# 2. Upload chunk
curl -X PUT "https://your-worker.workers.dev/api/upload/chunk" \
  -H "Content-Type: application/octet-stream" \
  -H "X-Upload-Id: ${UPLOAD_ID}" \
  -H "X-Chunk-Index: 0" \
  -d "Hello, World!"

# 3. Complete upload
curl -X POST "https://your-worker.workers.dev/api/upload/complete" \
  -H "Content-Type: application/json" \
  -d "{\"upload_id\": \"${UPLOAD_ID}\"}"

# 4. Check status
curl -X GET "https://your-worker.workers.dev/api/upload/${UPLOAD_ID}/status"
```

## Troubleshooting

### Common Issues

**Upload Fails with 413 Status**
- Check file size against configured maximum (default 10GB)
- Verify `total_size` matches actual file size

**Chunk Upload Returns 404**
- Verify upload was properly initialized
- Check `X-Upload-Id` header matches returned `upload_id`
- Ensure upload hasn't been cancelled or completed

**Complete Upload Fails with 400**
- Verify all chunks have been uploaded successfully
- Check upload status before attempting completion

**Database Errors (502)**
- Verify D1 database is properly configured in wrangler.toml
- Check database schema is applied correctly
- Monitor D1 database metrics in Cloudflare dashboard

**Performance Issues**
- Adjust chunk size based on network conditions
- Implement retry logic for failed chunk uploads
- Use parallel chunk uploads for better throughput
- Monitor R2 and D1 performance metrics

### Monitoring

- Use `wrangler tail` for real-time log monitoring
- Check Cloudflare Analytics for request metrics
- Monitor D1 database query performance
- Track R2 storage operations and costs

For additional support, consult the Cloudflare Workers documentation and community forums.