# Architecture Documentation

## Overview

MemeNow Storage is a high-performance, edge-based file storage service built with Rust and Cloudflare Workers. It provides robust chunked upload capabilities for large files using R2 storage, D1 database for state management, and KV storage for configuration.

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
│                  Storage & State                           │
├─────────────────────────────────────────────────────────────┤
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────┐  │
│  │ D1 Database  │  │  R2 Object   │  │   KV Storage     │  │
│  │  (Metadata)  │  │  Storage     │  │ (Configuration)  │  │
│  └──────────────┘  └──────────────┘  └──────────────────┘  │
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

### 3. Handlers Layer (`src/handlers/`)
- **Primary Function**: Business logic coordination
- **Responsibilities**:
  - Upload operation delegation to D1 DatabaseService
  - Health check endpoint implementation
  - Error response handling
  - CORS header application to responses

### 4. Database Layer (`src/database.rs`)
- **Primary Function**: Persistent upload state management via D1
- **DatabaseService Responsibilities**:
  - Upload CRUD operations (create, read, update, delete)
  - Chunk progress tracking with upsert semantics
  - Status lifecycle management
  - User upload listing with optional status filtering
  - Row deserialization with timestamp and enum parsing

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
1. Client → POST /api/upload/init
2. Router → CORS check → Upload handler
3. Handler → Validate request → R2.create_multipart_upload()
4. Handler → DatabaseService.create_upload() → Persist metadata in D1
5. Response → Upload ID + R2 key + chunk_size
```

### Chunk Upload Flow
```
1. Client → PUT /api/upload/chunk + headers
2. Router → Validation middleware → Upload handler
3. Handler → DatabaseService.get_upload() → Load metadata from D1
4. Handler → Validate state → R2.upload_part()
5. Handler → DatabaseService.record_chunk() → Update progress in D1
6. Response → Chunk confirmation + ETag
```

### Upload Completion Flow
```
1. Client → POST /api/upload/complete
2. Router → Upload handler
3. Handler → DatabaseService.get_upload() → Load metadata + chunks from D1
4. Handler → R2.complete_multipart_upload()
5. Handler → DatabaseService.update_upload_status() → Mark completed in D1
6. Response → Completion confirmation + R2 key
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
- **D1 ACID Compliance**: Upload operations are transactionally consistent
- **Metadata Protection**: Upload metadata stored in D1 with foreign key constraints
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
- **Not Found Errors (404)**: Missing uploads
- **Conflict Errors (409)**: Upload state conflicts (already completed / cancelled)
- **Server Errors (500)**: D1 database failures and internal errors
- **Upstream Errors (502)**: R2 storage failures

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
- D1 query operation logging
- Error context preservation

### Metrics (Future)
- Upload success/failure rates
- Chunk upload performance
- Storage utilization
- Error rate tracking

## Scalability Considerations

### Horizontal Scaling
- **Edge Distribution**: Automatic global scaling via Cloudflare
- **D1 Database Scaling**: Cloudflare-managed SQL with automatic replication
- **R2 Scaling**: Virtually unlimited storage capacity

### Vertical Scaling
- **Memory Efficiency**: Streaming chunk processing
- **CPU Efficiency**: Minimal processing per chunk
- **Storage Efficiency**: Direct R2 integration

### Limits and Constraints
- **File Size**: 10GB maximum (configurable)
- **Chunk Size**: 150MB default (configurable)
- **Concurrent Uploads**: Limited by client implementation
- **D1 Limits**: Per Cloudflare's D1 database constraints

## Deployment Architecture

### Infrastructure Components
- **Cloudflare Workers**: Serverless execution environment
- **D1 Database**: SQL database for upload metadata and state management
- **R2 Storage**: Object storage for files
- **KV Storage**: Configuration and metadata storage

### Environment Configuration
- **Development**: Local wrangler dev environment
- **Staging**: Preview deployments with preview resources
- **Production**: Production workers with production resources

### Resource Bindings
- `STORAGE_BUCKET`: R2 bucket binding for file storage
- `STORAGE_CONFIG`: KV namespace for configuration
- `UPLOAD_DB`: D1 database binding for upload metadata

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
- **Batch Operations**: Bulk upload management and cleanup
- **Caching Layer**: Upload metadata caching
- **Load Balancing**: Advanced request distribution
- **Multi-Region**: Cross-region replication

This architecture provides a solid foundation for a production-ready file storage service with room for future enhancements and scaling requirements.