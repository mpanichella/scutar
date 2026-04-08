use scutar_core::{ConnectionSpec, Result, StorageBackend};
use std::sync::Arc;

/// Build a `StorageBackend` from a `ConnectionSpec`. This is the single entry
/// point used by the engine layer; adding a new cloud is just a new arm here
/// plus a new submodule.
pub async fn build_backend(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Local { .. } => crate::local::build(spec).await,
        ConnectionSpec::S3 { .. } => crate::s3::build(spec).await,
        ConnectionSpec::Azure { .. } => crate::azure::build(spec).await,
        ConnectionSpec::Gcs { .. } => crate::gcs::build(spec).await,
        ConnectionSpec::Sftp { .. } => crate::sftp::build(spec).await,
    }
}
