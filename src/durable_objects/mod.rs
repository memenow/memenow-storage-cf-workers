//! # Durable Objects Module
//!
//! This module contains Durable Object implementations for the file storage service.
//! Durable Objects provide strongly consistent, persistent state management that
//! survives worker restarts and ensures data consistency across distributed deployments.
//!
//! ## Current Implementations
//!
//! - **UploadTracker**: Manages upload session state and coordinates multipart uploads
//!
//! ## Durable Objects Benefits
//!
//! - **Strong Consistency**: All operations on the same object are strongly consistent
//! - **Persistence**: State survives worker restarts and deployments
//! - **Isolation**: Each upload session gets its own isolated state
//! - **Concurrency**: Safe concurrent access to upload operations
//!
//! ## Usage Pattern
//!
//! ```rust
//! // Access from Worker
//! let namespace = env.durable_object("UPLOAD_TRACKER")?;
//! let id = namespace.id_from_name("UPLOAD_TRACKER")?;
//! let stub = id.get_stub()?;
//! let response = stub.fetch_with_request(req).await?;
//! ```

pub mod upload_tracker;