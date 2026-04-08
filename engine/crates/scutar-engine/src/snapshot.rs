//! Snapshot mode — content-addressable deduplicated backups.
//!
//! High-level pipeline:
//!
//! 1. Load (or initialize) the repo `config.json` on the destination.
//!    Initializes encryption header if `spec.encryption.enabled = true`.
//! 2. Load all existing pack indexes into a single in-memory dedup table
//!    `chunk_id -> PackIndexEntry`. This lets us skip uploading chunks we
//!    already have.
//! 3. Walk the source tree.
//! 4. For each file: read into memory, FastCDC-chunk, BLAKE3-hash. Skip
//!    chunks already in the dedup table; queue new ones into the packer.
//! 5. Flush packs as they fill; persist the index file with all new entries.
//! 6. Write the snapshot manifest.
//! 7. Apply retention if configured (prune old snapshots).
//!
//! The implementation prioritizes correctness and clarity over throughput;
//! parallel chunking + parallel uploads are an obvious follow-up.

use bytes::Bytes;
use futures::StreamExt;
use scutar_core::{
    repo_layout::{self, REPO_CONFIG_KEY},
    BackupSpec, Error, Result, StorageBackend,
};
use std::collections::BTreeMap;
use std::sync::Arc;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::chunker::{self, ChunkerParams};
use crate::encryption::{self, Sealer};
use crate::manifest::{
    ChunkRef, FileEntry, PackIndex, PackIndexEntry, RepoConfig, SnapshotManifest,
};
use crate::packer::Packer;
use crate::report::RunReport;
use crate::retention;
use crate::walker::Walker;

pub async fn run(spec: &BackupSpec, backend: Arc<dyn StorageBackend>) -> Result<RunReport> {
    // 1. Load or initialize repo config + sealer.
    let (config, sealer) = load_or_init_repo(spec, &backend).await?;
    tracing::info!(encrypted = config.encryption.is_some(), "snapshot: repo ready");

    // 2. Load existing dedup index.
    let dedup_table = load_dedup_table(&backend, &sealer).await?;
    tracing::info!(known_chunks = dedup_table.len(), "snapshot: dedup table loaded");

    // 3. Walk source.
    let walker = Walker::from_source(&spec.source)?;
    let files = walker.collect()?;
    tracing::info!(files = files.len(), "snapshot: walked source");

    // 4. Chunk + pack + upload.
    let mut packer = Packer::new(config.pack_target_size);
    let chunker_params = ChunkerParams::from_repo_config(&config);
    let mut report = RunReport {
        mode: "snapshot".to_string(),
        ..Default::default()
    };
    let mut manifest_files = Vec::<FileEntry>::with_capacity(files.len());
    let mut new_index_entries = Vec::<PackIndexEntry>::new();

    for file in &files {
        let bytes = tokio::fs::read(&file.abs_path).await?;
        let bytes_len = bytes.len() as u64;
        report.bytes_read += bytes_len;
        let chunks = chunker::chunk_bytes(Bytes::from(bytes), &chunker_params);

        let mut chunk_refs = Vec::<ChunkRef>::with_capacity(chunks.len());
        for chunk in chunks {
            chunk_refs.push(ChunkRef {
                id: chunk.id.clone(),
                size: chunk.data.len() as u32,
            });

            if dedup_table.contains_key(&chunk.id) {
                report.files_skipped += 0; // bookkeeping only
                continue;
            }

            let should_flush = packer.add(&chunk);
            if should_flush {
                let entries_before = packer.index.entries.len();
                packer.flush(&backend, &sealer).await?;
                let new = packer.index.entries[entries_before..].to_vec();
                new_index_entries.extend(new);
            }
        }

        manifest_files.push(FileEntry {
            path: file.rel_path.clone(),
            size: file.size,
            mtime: file.mtime,
            mode: None,
            chunks: chunk_refs,
        });
        report.files_processed += 1;
    }

    // Flush trailing pack.
    let entries_before = packer.index.entries.len();
    packer.flush(&backend, &sealer).await?;
    let new = packer.index.entries[entries_before..].to_vec();
    new_index_entries.extend(new);

    report.bytes_written += packer.bytes_written;

    // 5. Persist index file (only if we uploaded any new chunks).
    if !new_index_entries.is_empty() {
        let index = PackIndex {
            entries: new_index_entries,
        };
        let raw = serde_json::to_vec(&index)
            .map_err(|e| Error::Serde(format!("index: {e}")))?;
        let sealed = sealer.seal(Bytes::from(raw))?;
        let id = blake3_hex(&sealed);
        let key = repo_layout::index_key(&id);
        backend.put(&key, sealed).await?;
        tracing::info!(index_id = %id, "snapshot: index persisted");
    }

    // 6. Build + persist manifest.
    let now = OffsetDateTime::now_utc();
    let timestamp = now
        .format(&Rfc3339)
        .map_err(|e| Error::Other(format!("time format: {e}")))?;
    let bytes_total: u64 = manifest_files.iter().map(|f| f.size).sum();
    let labels: BTreeMap<String, String> = spec.labels.clone().into_iter().collect();
    let mut manifest = SnapshotManifest {
        id: String::new(),
        created_at: timestamp.clone(),
        backup_name: spec.name.clone(),
        source_root: spec.source.path.to_string_lossy().into_owned(),
        bytes_total,
        files: manifest_files,
        labels,
    };
    let manifest_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| Error::Serde(format!("manifest: {e}")))?;
    let manifest_id = blake3_hex(&Bytes::copy_from_slice(&manifest_bytes));
    manifest.id = manifest_id.clone();

    let final_bytes = serde_json::to_vec(&manifest)
        .map_err(|e| Error::Serde(format!("manifest final: {e}")))?;
    let sealed_manifest = sealer.seal(Bytes::from(final_bytes))?;
    let key = repo_layout::snapshot_key(&timestamp, repo_layout::short_id(&manifest_id));
    backend.put(&key, sealed_manifest).await?;
    report.snapshot_id = Some(manifest_id.clone());
    tracing::info!(snapshot_id = %manifest_id, "snapshot: manifest persisted");

    // 7. Retention.
    if let Some(policy) = &spec.retention {
        let pruned = retention::apply(&backend, &sealer, policy).await?;
        if pruned > 0 {
            tracing::info!(pruned, "snapshot: retention applied");
        }
    }

    Ok(report)
}

async fn load_or_init_repo(
    spec: &BackupSpec,
    backend: &Arc<dyn StorageBackend>,
) -> Result<(RepoConfig, Sealer)> {
    match backend.get(REPO_CONFIG_KEY).await {
        Ok(bytes) => {
            // Existing repo: parse, then derive sealer if needed.
            let config: RepoConfig = serde_json::from_slice(&bytes)
                .map_err(|e| Error::Serde(format!("repo config: {e}")))?;
            let sealer = match (&config.encryption, &spec.encryption) {
                (Some(header), Some(enc)) if enc.enabled => {
                    let password = encryption::read_password_file(&enc.password_file)?;
                    encryption::open_encryption(&password, header)?
                }
                (None, Some(enc)) if enc.enabled => {
                    return Err(Error::Config(
                        "spec requires encryption but repo is unencrypted".into(),
                    ));
                }
                (Some(_), _) => {
                    return Err(Error::Config(
                        "repo is encrypted but spec.encryption.enabled is false (need password)".into(),
                    ));
                }
                _ => Sealer::none(),
            };
            Ok((config, sealer))
        }
        Err(Error::NotFound(_)) => {
            // Brand new repo: initialize.
            let mut config = RepoConfig::new_default();
            let sealer = if let Some(enc) = &spec.encryption {
                if enc.enabled {
                    let password = encryption::read_password_file(&enc.password_file)?;
                    let (sealer, header) = encryption::init_encryption(&password)?;
                    config.encryption = Some(header);
                    sealer
                } else {
                    Sealer::none()
                }
            } else {
                Sealer::none()
            };
            let raw = serde_json::to_vec_pretty(&config)
                .map_err(|e| Error::Serde(format!("repo config write: {e}")))?;
            backend.put(REPO_CONFIG_KEY, Bytes::from(raw)).await?;
            Ok((config, sealer))
        }
        Err(e) => Err(e),
    }
}

async fn load_dedup_table(
    backend: &Arc<dyn StorageBackend>,
    sealer: &Sealer,
) -> Result<BTreeMap<String, PackIndexEntry>> {
    let mut table = BTreeMap::new();
    let mut stream = backend.list("index/");
    let mut keys = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(meta) => keys.push(meta.key),
            Err(e) => return Err(e),
        }
    }
    drop(stream);
    for key in keys {
        let raw = backend.get(&key).await?;
        let opened = sealer.open(raw)?;
        let index: PackIndex = serde_json::from_slice(&opened)
            .map_err(|e| Error::Serde(format!("index parse: {e}")))?;
        for entry in index.entries {
            table.insert(entry.chunk_id.clone(), entry);
        }
    }
    Ok(table)
}

fn blake3_hex(bytes: &Bytes) -> String {
    let h = blake3::hash(bytes);
    let bytes = h.as_bytes();
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}
