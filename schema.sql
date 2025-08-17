-- D1 Database Schema for MemeNow Storage Service
-- This schema replaces Durable Objects with D1 database storage
-- Created: 2025-08-17

-- Upload metadata table
-- Stores comprehensive information about file uploads
CREATE TABLE uploads (
    -- Primary key: Unique upload identifier (UUID v4)
    upload_id TEXT PRIMARY KEY,
    
    -- File information
    file_name TEXT NOT NULL,
    total_size INTEGER NOT NULL,
    content_type TEXT NOT NULL,
    
    -- User information
    user_id TEXT NOT NULL,
    user_role TEXT NOT NULL CHECK (user_role IN ('creator', 'member', 'subscriber')),
    
    -- Storage information
    r2_key TEXT NOT NULL,
    r2_upload_id TEXT NOT NULL,
    
    -- Status tracking
    status TEXT NOT NULL CHECK (status IN ('initiated', 'in_progress', 'completed', 'cancelled')),
    
    -- Timestamp tracking (ISO 8601 format)
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Upload chunks table
-- Tracks individual chunks for multipart uploads
CREATE TABLE upload_chunks (
    -- Composite primary key
    upload_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    
    -- Chunk metadata
    chunk_size INTEGER NOT NULL,
    etag TEXT,  -- R2 ETag for the chunk
    uploaded_at TEXT NOT NULL,
    
    PRIMARY KEY (upload_id, chunk_index),
    FOREIGN KEY (upload_id) REFERENCES uploads(upload_id) ON DELETE CASCADE
);

-- Create indexes for performance optimization
CREATE INDEX idx_uploads_user_id ON uploads(user_id);
CREATE INDEX idx_uploads_status ON uploads(status);
CREATE INDEX idx_uploads_created_at ON uploads(created_at);
CREATE INDEX idx_uploads_user_role ON uploads(user_role);
CREATE INDEX idx_upload_chunks_upload_id ON upload_chunks(upload_id);

-- Upload statistics view
-- Provides aggregated statistics for monitoring
CREATE VIEW upload_stats AS
SELECT 
    user_role,
    status,
    COUNT(*) as upload_count,
    SUM(total_size) as total_bytes,
    AVG(total_size) as avg_file_size,
    MIN(created_at) as earliest_upload,
    MAX(created_at) as latest_upload
FROM uploads 
GROUP BY user_role, status;

-- Active uploads view
-- Shows uploads currently in progress
CREATE VIEW active_uploads AS
SELECT 
    upload_id,
    file_name,
    user_id,
    user_role,
    total_size,
    status,
    (SELECT COUNT(*) FROM upload_chunks WHERE upload_chunks.upload_id = uploads.upload_id) as chunks_uploaded,
    created_at,
    updated_at
FROM uploads 
WHERE status IN ('initiated', 'in_progress')
ORDER BY created_at DESC;