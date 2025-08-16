# Architecture Documentation

## Overview

MemeNow Storage is a high-performance, edge-based file storage service built with Rust and Cloudflare Workers. It provides robust chunked upload capabilities for large files using R2 storage, Durable Objects for state management, and KV storage for configuration.

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Internet/Client                         │
└─────────────────────┬───────────────────────────────────────┘
                      │ HTTP/HTTPS Requests
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                Cloudflare Edge Network                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   Router    │  │ Middleware  │  │      Handlers       │ │
│  │   Layer     │→ │   Layer     │→ │       Layer         │ │
│  └─────────────┘  └─────────────┘  └─────────────────────┘ │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                Durable Objects                             │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────────────────────────────────────────────┐ │
│  │           Upload Tracker Instance                      │ │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │ │
│  │  │   Upload    │  │   Chunk     │  │   R2 Upload     │ │ │
│  │  │ Initiation  │  │ Processing  │  │  Coordination   │ │ │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘ │ │
│  └─────────────────────────────────────────────────────────┘ │
└─────────────────────┬───────────────────────────────────────┘
                      │
                      ▼
┌─────────────────────────────────────────────────────────────┐
│                Storage & Configuration                     │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐  ┌─────────────────────────────────────┐ │
│  │   KV Storage    │  │          R2 Object Storage          │ │
│  │ (Configuration) │  │        (File Storage)               │ │
│  └─────────────────┘  └─────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Component Responsibilities

### 1. Router Layer (`src/router.rs`)
- **Primary Function**: HTTP request routing and dispatch
- **Responsibilities**:
  - Route matching based on method + path patterns
  - CORS preflight handling
  - Request delegation to appropriate handlers
  - 404 handling for unmatched routes

### 2. Middleware Layer (`src/middleware.rs`)
- **Primary Function**: Cross-cutting request/response processing
- **Components**:
  - **CORS Middleware**: Cross-origin request support
  - **Validation Middleware**: Request validation and sanitization
- **Responsibilities**:
  - Header validation (X-Upload-Id, X-Chunk-Index)
  - File size validation
  - Content type validation
  - CORS header application

### 3. Handlers Layer (`src/handlers.rs`)
- **Primary Function**: Business logic coordination
- **Responsibilities**:
  - Upload operation delegation to Durable Objects
  - Health check endpoint implementation
  - Error response handling
  - CORS header application to responses

### 4. Durable Objects (`src/durable_objects/`)
- **Primary Function**: Stateful upload session management
- **UploadTracker Responsibilities**:
  - Upload session lifecycle management
  - R2 multipart upload coordination
  - Chunk progress tracking
  - State persistence across worker restarts
  - Concurrent operation safety

### 5. Models Layer (`src/models.rs`)
- **Primary Function**: Data structure definitions
- **Key Types**:
  - `UserRole`: User role enumeration (creator/member/subscriber)
  - `UploadMetadata`: Complete upload session information
  - `UploadStatus`: Upload lifecycle state tracking

### 6. Configuration (`src/config.rs`)
- **Primary Function**: Runtime configuration management
- **Features**:
  - KV-based configuration with defaults
  - Runtime parameter adjustment
  - Upload limits and chunk sizes

### 7. Error Handling (`src/errors.rs`)
- **Primary Function**: Comprehensive error management
- **Features**:
  - Structured error types with context
  - Automatic HTTP status code mapping
  - JSON error response generation
  - Integration with all system components

### 8. Utilities (`src/utils.rs`)
- **Primary Function**: Shared utility functions
- **Features**:
  - R2 key generation with hierarchical organization
  - Cryptographically secure upload ID generation
  - CORS header standardization

## Data Flow

### Upload Initialization Flow
```
1. Client → POST /v1/uploads/init
2. Router → CORS check → Upload handler
3. Handler → Durable Object (UploadTracker)
4. UploadTracker → Validate request → Create metadata
5. UploadTracker → R2.create_multipart_upload()
6. UploadTracker → Store metadata in DO storage
7. Response → Upload ID + R2 key
```

### Chunk Upload Flow
```
1. Client → POST /v1/uploads/{id}/chunk + headers
2. Router → Validation middleware → Upload handler
3. Handler → Durable Object (UploadTracker)
4. UploadTracker → Load metadata → Validate state
5. UploadTracker → R2.upload_part()
6. UploadTracker → Update chunk progress → Save metadata
7. Response → Chunk confirmation + ETag
```

### Upload Completion Flow
```
1. Client → POST /v1/uploads/{id}/complete + parts list
2. Router → Upload handler → Durable Object
3. UploadTracker → Load metadata → Validate parts
4. UploadTracker → R2.complete_multipart_upload()
5. UploadTracker → Update status to Completed
6. Response → Completion confirmation
```

## File Organization Strategy

Files are organized in R2 storage using a hierarchical structure:

```
{userRole}/{userId}/{date}/{contentCategory}/{fileName}
```

### Example Paths
- `creator/user123/20240115/image/profile.jpg`
- `member/user456/20240115/video/presentation.mp4`
- `subscriber/user789/20240115/document/report.pdf`

### Content Categories
- `image/` - Image files (JPEG, PNG, GIF, WebP)
- `video/` - Video files (MP4, AVI, MOV, WebM)
- `audio/` - Audio files (MP3, WAV, AAC, OGG)
- `document/` - Text and document files (PDF, TXT, JSON)
- `other/` - All other file types

## Security Architecture

### Upload Security
- **File Size Limits**: Configurable maximum file size (default: 10GB)
- **Content Type Validation**: Whitelist of allowed MIME types
- **Unique Identifiers**: Cryptographically secure upload session IDs
- **Chunk Validation**: ETag verification for uploaded chunks

### CORS Configuration
- **Origin Policy**: Currently allows all origins (`*`)
- **Methods**: GET, POST, PUT, DELETE, OPTIONS
- **Headers**: Content-Type, X-Upload-Id, X-Chunk-Index

### State Security
- **Durable Object Isolation**: Each upload session isolated
- **Metadata Protection**: Upload metadata stored in DO storage
- **R2 Integration**: Secure coordination with R2 multipart uploads

## Performance Characteristics

### Edge Performance
- **Global Distribution**: Deployed to Cloudflare's edge network
- **Sub-50ms Latency**: Worldwide response times
- **Auto-scaling**: Zero cold starts with V8 isolation

### Upload Performance
- **Chunked Uploads**: 150MB default chunk size
- **Parallel Processing**: Support for concurrent chunk uploads
- **Resumable Uploads**: State persistence enables resume capability
- **Large File Support**: Up to 10GB file uploads

### Storage Performance
- **R2 Integration**: High-performance object storage
- **Multipart Uploads**: Efficient handling of large files
- **Global Availability**: R2's global distribution

## Error Handling Strategy

### Error Categories
- **Client Errors (4xx)**: Invalid input, missing fields, file size limits
- **Authentication Errors (401)**: Future authentication failures
- **Not Found Errors (404)**: Missing uploads or invalid endpoints
- **Conflict Errors (409)**: Upload state conflicts
- **Server Errors (5xx)**: Internal failures, storage errors
- **Service Errors (502)**: External service failures (R2, KV)

### Error Response Format
```json
{
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable description",
    "timestamp": "2024-01-15T10:30:00Z"
  }
}
```

## Monitoring and Observability

### Health Endpoints
- `GET /health` - Service health status
- Returns: Service identification, status, timestamp

### Logging
- Request/response logging via `console_log!`
- Operation tracing in Durable Objects
- Error context preservation

### Metrics (Future)
- Upload success/failure rates
- Chunk upload performance
- Storage utilization
- Error rate tracking

## Scalability Considerations

### Horizontal Scaling
- **Edge Distribution**: Automatic global scaling via Cloudflare
- **Durable Object Scaling**: Isolated state per upload session
- **R2 Scaling**: Virtually unlimited storage capacity

### Vertical Scaling
- **Memory Efficiency**: Streaming chunk processing
- **CPU Efficiency**: Minimal processing per chunk
- **Storage Efficiency**: Direct R2 integration

### Limits and Constraints
- **File Size**: 10GB maximum (configurable)
- **Chunk Size**: 150MB default (configurable)
- **Concurrent Uploads**: Limited by client implementation
- **Durable Object Limits**: Per Cloudflare's constraints

## Deployment Architecture

### Infrastructure Components
- **Cloudflare Workers**: Serverless execution environment
- **Durable Objects**: Stateful computing for upload sessions
- **R2 Storage**: Object storage for files
- **KV Storage**: Configuration and metadata storage

### Environment Configuration
- **Development**: Local wrangler dev environment
- **Staging**: Preview deployments with preview resources
- **Production**: Production workers with production resources

### Resource Bindings
- `BUCKET`: R2 bucket binding for file storage
- `CONFIG`: KV namespace for configuration
- `UPLOAD_TRACKER`: Durable Object binding for state management

## Future Enhancements

### Potential Improvements
- **Authentication**: User authentication and authorization
- **Access Control**: Fine-grained permission system
- **File Encryption**: Client-side or server-side encryption
- **Thumbnail Generation**: Automatic image/video thumbnails
- **CDN Integration**: Content delivery optimization
- **Analytics**: Upload and usage analytics
- **Webhook Support**: Upload completion notifications

### Scalability Enhancements
- **Database Integration**: Metadata storage in databases
- **Caching Layer**: Upload metadata caching
- **Load Balancing**: Advanced request distribution
- **Multi-Region**: Cross-region replication

This architecture provides a solid foundation for a production-ready file storage service with room for future enhancements and scaling requirements.