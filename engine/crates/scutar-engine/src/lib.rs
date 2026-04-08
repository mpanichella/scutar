//! Scutar engine: the high-level orchestration layer that turns a `BackupSpec`
//! into actual work against a `StorageBackend`.
//!
//! Two execution modes:
//!
//! * `snapshot` — content-addressable, deduplicated, optionally encrypted.
//!   Walks the source tree, splits files into content-defined chunks
//!   (FastCDC + BLAKE3), packs small chunks together, uploads packs and a
//!   snapshot manifest. Restorable point-in-time backups with retention.
//!
//! * `mirror` — 1:1 mirror of source against destination. No history,
//!   no chunking, no dedup. Equivalent to rclone sync.
//!
//! Both modes consume the same `dyn StorageBackend`, so adding a new cloud
//! gets both modes for free.

pub mod chunker;
pub mod encryption;
pub mod manifest;
pub mod mirror;
pub mod packer;
pub mod report;
pub mod restore;
pub mod retention;
pub mod snapshot;
pub mod walker;

use scutar_core::{BackupMode, BackupSpec, Result};

pub use report::RunReport;

/// Run a backup according to the given spec. The engine resolves the backend
/// from the connection spec, dispatches to the right mode, and returns when
/// the run is fully durable on the destination.
pub async fn run(spec: &BackupSpec) -> Result<RunReport> {
    let backend = scutar_backends::build_backend(&spec.destination).await?;
    match spec.mode {
        BackupMode::Snapshot => snapshot::run(spec, backend).await,
        BackupMode::Mirror => mirror::run(spec, backend).await,
    }
}

// Re-export so the CLI doesn't need a direct dependency on backends.
pub use scutar_backends::build_backend;
pub use scutar_core::StorageBackend;
