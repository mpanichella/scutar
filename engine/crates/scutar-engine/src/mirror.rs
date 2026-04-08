//! Mirror mode — 1:1 sync of source to destination, no history.
//!
//! Algorithm:
//!   1. Walk the local source tree honoring include/exclude globs.
//!   2. Fetch the remote state file (`.scutar-mirror-state.json`) if present.
//!      It maps `relpath -> { size, mtime, blake3 }` from the previous run.
//!   3. For each local file, compute BLAKE3 only if (size, mtime) changed
//!      relative to the cached state. This avoids re-hashing terabytes of
//!      unchanged data on every run.
//!   4. Upload any local file whose hash differs from the remote state
//!      (or that's new).
//!   5. Delete remote files that no longer exist locally (`--delete`).
//!   6. Persist the new state file.

use bytes::Bytes;
use scutar_core::{
    repo_layout::MIRROR_STATE_KEY, BackupSpec, Error, Result, StorageBackend,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

use crate::report::RunReport;
use crate::walker::Walker;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct MirrorState {
    /// relpath -> entry
    files: BTreeMap<String, MirrorEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MirrorEntry {
    size: u64,
    mtime: Option<i64>,
    /// Lowercase hex BLAKE3.
    hash: String,
}

pub async fn run(spec: &BackupSpec, backend: Arc<dyn StorageBackend>) -> Result<RunReport> {
    let walker = Walker::from_source(&spec.source)?;
    let local = walker.collect()?;
    tracing::info!(files = local.len(), "mirror: walked source");

    let mut state = load_state(&*backend).await?;
    let mut new_state = MirrorState::default();
    let mut report = RunReport {
        mode: "mirror".to_string(),
        ..Default::default()
    };

    for file in &local {
        let cached = state.files.remove(&file.rel_path);
        let needs_hash = match &cached {
            Some(c) => c.size != file.size || c.mtime != file.mtime,
            None => true,
        };

        let hash = if needs_hash {
            let bytes = tokio::fs::read(&file.abs_path).await?;
            let h = blake3::hash(&bytes);
            let hex = hex(h.as_bytes());

            let upload_needed = match &cached {
                Some(c) => c.hash != hex,
                None => true,
            };
            if upload_needed {
                backend
                    .put(&file.rel_path, Bytes::from(bytes))
                    .await?;
                report.bytes_written += file.size;
                tracing::debug!(file = %file.rel_path, "mirror: uploaded");
            } else {
                report.files_skipped += 1;
            }
            report.bytes_read += file.size;
            hex
        } else {
            // Cached hit: nothing to do.
            report.files_skipped += 1;
            cached.as_ref().unwrap().hash.clone()
        };

        new_state.files.insert(
            file.rel_path.clone(),
            MirrorEntry {
                size: file.size,
                mtime: file.mtime,
                hash,
            },
        );
        report.files_processed += 1;
    }

    // Anything still in `state.files` was not seen locally — delete from remote.
    for (orphan_path, _) in state.files.iter() {
        backend.delete(orphan_path).await?;
        report.files_deleted += 1;
        tracing::debug!(file = %orphan_path, "mirror: deleted orphan");
    }

    // Persist the new state.
    let raw = serde_json::to_vec_pretty(&new_state)
        .map_err(|e| Error::Serde(format!("mirror state: {e}")))?;
    backend
        .put(MIRROR_STATE_KEY, Bytes::from(raw))
        .await?;

    tracing::info!(
        bytes_read = report.bytes_read,
        bytes_written = report.bytes_written,
        deleted = report.files_deleted,
        skipped = report.files_skipped,
        "mirror: done"
    );
    Ok(report)
}

async fn load_state(backend: &dyn StorageBackend) -> Result<MirrorState> {
    match backend.get(MIRROR_STATE_KEY).await {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Serde(format!("mirror state parse: {e}"))),
        Err(Error::NotFound(_)) => Ok(MirrorState::default()),
        Err(e) => Err(e),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

