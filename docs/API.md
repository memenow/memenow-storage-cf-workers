# API Documentation

## Overview

The MemeNow Storage API provides a REST interface for managing large file uploads using chunked multipart uploads. The API is built on Cloudflare Workers and provides global edge performance with strong consistency guarantees.

## Base URL

```text
https://your-worker-name.your-subdomain.workers.dev
```

## Authentication

Currently, the API uses user identification via the `userId` parameter in requests. Future versions will include token-based authentication.

## Rate Limiting

Rate limiting should be implemented at the Cloudflare dashboard level or via custom middleware for production deployments.

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
POST /v1/uploads/init
Content-Type: application/json
```

#### Initialize Upload Request Body

```json
{
  "fileName": "example.mp4",
  "totalSize": 524288000,
  "userRole": "creator",
  "contentType": "video/mp4",
  "userId": "user_12345"
}
```

##### Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `fileName` | string | Yes | Original filename |
| `totalSize` | number | Yes | Total file size in bytes |
| `userRole` | string | Yes | User role: `creator`, `member`, or `subscriber` |
| `contentType` | string | Yes | MIME type of the file |
| `userId` | string | Yes | Unique user identifier |

#### Initialize Upload Response

```json
{
  "message": "Multipart upload initiated",
  "uploadId": "1704447000000-550e8400-e29b-41d4-a716-446655440000-12345678901234567890",
  "r2Key": "creator/user_12345/20240105/video/example.mp4"
}
```

**Status Codes:**
- `200` - Upload initialized successfully
- `400` - Invalid request parameters
- `413` - File size exceeds maximum allowed

---

### Upload Chunk

Upload a chunk of the file.

```http
POST /v1/uploads/{uploadId}/chunk
Content-Type: application/octet-stream
X-Upload-Id: {uploadId}
X-Chunk-Index: {chunkNumber}
```

#### Upload Chunk Headers

| Header | Type | Required | Description |
|--------|------|----------|-------------|
| `X-Upload-Id` | string | Yes | Upload session identifier |
| `X-Chunk-Index` | number | Yes | Chunk number (starting from 1) |
| `Content-Type` | string | Yes | Must be `application/octet-stream` |

#### Upload Chunk Request Body

Binary data representing the file chunk.

#### Upload Chunk Response

```json
{
  "message": "Chunk uploaded successfully",
  "chunkIndex": 1,
  "etag": "\"9bb58f26192e4ba00f01e2e7b136bbd8\"",
  "uploadId": "1704447000000-550e8400-e29b-41d4-a716-446655440000-12345678901234567890",
  "r2UploadId": "2~1234567890abcdef1234567890abcdef"
}
```

**Status Codes:**
- `200` - Chunk uploaded successfully
- `400` - Invalid headers or chunk data
- `404` - Upload session not found

---

### Complete Upload

Complete the multipart upload by providing all uploaded parts.

```http
POST /v1/uploads/{uploadId}/complete
Content-Type: application/json
```

#### Complete Upload Request Body

```json
{
  "uploadId": "1704447000000-550e8400-e29b-41d4-a716-446655440000-12345678901234567890",
  "parts": [
    {
      "etag": "\"9bb58f26192e4ba00f01e2e7b136bbd8\"",
      "partNumber": 1
    },
    {
      "etag": "\"1234567890abcdef1234567890abcdef\"",
      "partNumber": 2
    }
  ]
}
```

##### Complete Upload Request Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `uploadId` | string | Yes | Upload session identifier |
| `parts` | array | Yes | Array of uploaded parts with ETags |
| `parts[].etag` | string | Yes | ETag returned from chunk upload |
| `parts[].partNumber` | number | Yes | Part number (matches chunk index) |

#### Complete Upload Response

```json
{
  "message": "Multipart upload completed successfully",
  "uploadId": "1704447000000-550e8400-e29b-41d4-a716-446655440000-12345678901234567890"
}
```

**Status Codes:**
- `200` - Upload completed successfully
- `400` - Invalid parts list or missing parts
- `404` - Upload session not found

---

### Get Upload Status

Retrieve the current status of an upload session.

```http
GET /v1/uploads/{uploadId}
X-Upload-Id: {uploadId}
```

#### Get Upload Status Headers

| Header | Type | Required | Description |
|--------|------|----------|-------------|
| `X-Upload-Id` | string | Yes | Upload session identifier |

#### Get Upload Status Response

```json
{
  "uploadId": "1704447000000-550e8400-e29b-41d4-a716-446655440000-12345678901234567890",
  "fileName": "example.mp4",
  "totalSize": 524288000,
  "uploadedChunks": [1, 2, 3],
  "status": "InProgress"
}
```

##### Status Values

- `Initiated` - Upload session created but no chunks uploaded
- `InProgress` - Some chunks have been uploaded
- `Completed` - All chunks uploaded and file assembled
- `Cancelled` - Upload was cancelled

**Status Codes:**
- `200` - Status retrieved successfully
- `404` - Upload session not found

---

### Cancel Upload

Cancel an active upload session and clean up resources.

```http
DELETE /v1/uploads/{uploadId}
X-Upload-Id: {uploadId}
```

#### Cancel Upload Headers

| Header | Type | Required | Description |
|--------|------|----------|-------------|
| `X-Upload-Id` | string | Yes | Upload session identifier |

#### Cancel Upload Response

```json
{
  "message": "Upload cancelled successfully"
}
```

**Status Codes:**
- `200` - Upload cancelled successfully
- `404` - Upload session not found

## File Organization

Files are organized in R2 storage using a structured path format:

```text
{userRole}/{userId}/{date}/{contentCategory}/{fileName}
```

### Path Components

- **userRole**: `creator`, `member`, or `subscriber`
- **userId**: Unique user identifier
- **date**: Upload date in `YYYYMMDD` format
- **contentCategory**: Determined by content type:
  - `image` - Image files (image/*)
  - `video` - Video files (video/*)
  - `audio` - Audio files (audio/*)
  - `document` - Text and JSON files
  - `other` - Other file types

### Example Paths

```text
creator/user123/20240115/image/profile.jpg
member/user456/20240115/video/presentation.mp4
subscriber/user789/20240115/document/report.pdf
```

## Usage Examples

### JavaScript SDK Example

```javascript
class MemeNowStorageClient {
  constructor(baseUrl) {
    this.baseUrl = baseUrl;
  }

  async uploadFile(file, userId, userRole) {
    // Initialize upload
    const initResponse = await fetch(`${this.baseUrl}/v1/uploads/init`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        fileName: file.name,
        totalSize: file.size,
        userRole,
        contentType: file.type,
        userId
      })
    });

    const { uploadId } = await initResponse.json();

    // Upload chunks
    const chunkSize = 150 * 1024 * 1024; // 150MB
    const parts = [];
    
    for (let i = 0; i < file.size; i += chunkSize) {
      const chunk = file.slice(i, i + chunkSize);
      const chunkIndex = Math.floor(i / chunkSize) + 1;
      
      const chunkResponse = await fetch(`${this.baseUrl}/v1/uploads/${uploadId}/chunk`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/octet-stream',
          'X-Upload-Id': uploadId,
          'X-Chunk-Index': chunkIndex.toString()
        },
        body: chunk
      });
      
      const chunkResult = await chunkResponse.json();
      parts.push({
        etag: chunkResult.etag,
        partNumber: chunkIndex
      });
    }

    // Complete upload
    const completeResponse = await fetch(`${this.baseUrl}/v1/uploads/${uploadId}/complete`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ uploadId, parts })
    });

    return await completeResponse.json();
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

class MemeNowStorageClient:
    def __init__(self, base_url):
        self.base_url = base_url
    
    def upload_file(self, file_path, user_id, user_role):
        with open(file_path, 'rb') as f:
            file_data = f.read()
            file_size = len(file_data)
            file_name = file_path.split('/')[-1]
        
        # Initialize upload
        init_response = requests.post(
            f"{self.base_url}/v1/uploads/init",
            json={
                "fileName": file_name,
                "totalSize": file_size,
                "userRole": user_role,
                "contentType": "application/octet-stream",
                "userId": user_id
            }
        )
        
        upload_data = init_response.json()
        upload_id = upload_data["uploadId"]
        
        # Upload chunks
        chunk_size = 150 * 1024 * 1024  # 150MB
        parts = []
        
        for i in range(0, file_size, chunk_size):
            chunk_data = file_data[i:i + chunk_size]
            chunk_index = i // chunk_size + 1
            
            chunk_response = requests.post(
                f"{self.base_url}/v1/uploads/{upload_id}/chunk",
                data=chunk_data,
                headers={
                    "Content-Type": "application/octet-stream",
                    "X-Upload-Id": upload_id,
                    "X-Chunk-Index": str(chunk_index)
                }
            )
            
            chunk_result = chunk_response.json()
            parts.append({
                "etag": chunk_result["etag"],
                "partNumber": chunk_index
            })
        
        # Complete upload
        complete_response = requests.post(
            f"{self.base_url}/v1/uploads/{upload_id}/complete",
            json={"uploadId": upload_id, "parts": parts}
        )
        
        return complete_response.json()

# Usage
client = MemeNowStorageClient("https://your-worker.workers.dev")
result = client.upload_file("/path/to/file.mp4", "user123", "creator")
```

## Configuration

### Environment Variables

Configure the following in your Cloudflare Workers environment:

| Variable | Required | Description |
|----------|----------|-------------|
| `BUCKET` | Yes | R2 bucket binding name |
| `CONFIG` | Yes | KV namespace binding name |
| `UPLOAD_TRACKER` | Yes | Durable Object binding name |

### KV Configuration

Store configuration in KV under the key `config`:

```json
{
  "durable_object_name": "UPLOAD_TRACKER",
  "max_file_size": 10737418240,
  "chunk_size": 157286400
}
```

### Configuration Fields

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `durable_object_name` | string | "UPLOAD_TRACKER" | Durable Object binding name |
| `max_file_size` | number | 10737418240 | Maximum file size in bytes (10GB) |
| `chunk_size` | number | 157286400 | Recommended chunk size in bytes (150MB) |

## Testing

### Health Check Test

```bash
curl -X GET https://your-worker.workers.dev/health
```

### Upload Flow Test

```bash
# 1. Initialize upload
UPLOAD_ID=$(curl -X POST https://your-worker.workers.dev/v1/uploads/init \
  -H "Content-Type: application/json" \
  -d '{
    "fileName": "test.txt",
    "totalSize": 13,
    "userRole": "creator",
    "contentType": "text/plain",
    "userId": "test_user"
  }' | jq -r '.uploadId')

# 2. Upload chunk
ETAG=$(curl -X POST "https://your-worker.workers.dev/v1/uploads/${UPLOAD_ID}/chunk" \
  -H "Content-Type: application/octet-stream" \
  -H "X-Upload-Id: ${UPLOAD_ID}" \
  -H "X-Chunk-Index: 1" \
  -d "Hello, World!" | jq -r '.etag')

# 3. Complete upload
curl -X POST "https://your-worker.workers.dev/v1/uploads/${UPLOAD_ID}/complete" \
  -H "Content-Type: application/json" \
  -d "{
    \"uploadId\": \"${UPLOAD_ID}\",
    \"parts\": [{
      \"etag\": \"${ETAG}\",
      \"partNumber\": 1
    }]
  }"
```

## Troubleshooting

### Common Issues

**Upload Fails with 413 Status**

- Check file size against configured maximum
- Verify `totalSize` matches actual file size

**Chunk Upload Returns 404**

- Verify upload was properly initialized
- Check `X-Upload-Id` header matches returned `uploadId`

**Complete Upload Fails with 400**

- Ensure all chunks have been uploaded
- Verify ETags in parts array match chunk upload responses
- Check that part numbers are sequential starting from 1

**Performance Issues**

- Adjust chunk size based on network conditions
- Implement retry logic for failed chunk uploads
- Use parallel chunk uploads for better throughput

For additional support, check the server logs via `wrangler tail` command.