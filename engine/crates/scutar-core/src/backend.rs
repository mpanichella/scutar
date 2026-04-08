use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Metadata returned by a backend `list` call. Kept minimal so all backends
/// can populate it cheaply.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub key: String,
    pub size: u64,
    pub last_modified: Option<String>, // RFC3339; backend-agnostic
    pub etag: Option<String>,
}

/// Capabilities advertised by a backend. The engine uses these to pick the
/// optimal upload strategy (e.g. multipart vs single PUT).
#[derive(Debug, Clone, Copy, Default)]
pub struct BackendCapabilities {
    pub supports_multipart: bool,
    pub max_object_size: Option<u64>,
    pub supports_atomic_rename: bool,
}

/// Storage abstraction. Every cloud backend (S3, Azure Blob, GCS, SFTP, ...)
/// implements this trait. Adding a new backend means: implement this trait,
/// register it in the factory, done — no changes to the engine layer.
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// Stable backend identifier (e.g. "s3", "azure", "gcs", "sftp").
    fn name(&self) -> &str;

    /// Capabilities advertised by this backend.
    fn capabilities(&self) -> BackendCapabilities;

    /// Upload an object. Backends with `supports_multipart` should chunk
    /// internally based on size; the engine just hands over bytes.
    async fn put(&self, key: &str, data: Bytes) -> Result<()>;

    /// Download a full object. For very large objects use `get_range`.
    async fn get(&self, key: &str) -> Result<Bytes>;

    /// Download a byte range from an object.
    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes>;

    /// Check if an object exists.
    async fn exists(&self, key: &str) -> Result<bool>;

    /// List objects under a prefix. Returns a stream so backends can paginate
    /// transparently without buffering everything in memory.
    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>>;

    /// Delete an object. Idempotent: deleting a missing key is not an error.
    async fn delete(&self, key: &str) -> Result<()>;
}
