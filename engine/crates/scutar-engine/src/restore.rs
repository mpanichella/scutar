//! Restore — read a snapshot manifest from a repo and reconstruct the source
//! tree on a local filesystem path.
//!
//! Restore loads:
//!   * the repo `config.json` (to learn encryption parameters and chunker
//!     sizes — though chunker sizes don't matter for restore),
//!   * all `index/*` files to map `chunk_id -> (pack_id, offset, length)`,
//!   * the snapshot manifest by id.
//!
//! Then for each file in the manifest it fetches the chunks (deduplicated;
//! unique chunks are downloaded once and cached in a small in-memory LRU)
//! and writes the file to disk under `target_path`.

use futures::StreamExt;
use scutar_core::{
    repo_layout::{self, REPO_CONFIG_KEY},
    BackupSpec, Error, Result, StorageBackend,
};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::encryption::{self, Sealer};
use crate::manifest::{PackIndex, PackIndexEntry, RepoConfig, SnapshotManifest};
use crate::report::RunReport;

/// Public restore entry point used by the CLI.
pub async fn restore_from_spec(
    spec: &BackupSpec,
    snapshot_id: &str,
    target_path: &Path,
) -> Result<RunReport> {
    let backend = scutar_backends::build_backend(&spec.destination).await?;
    let sealer = open_repo_sealer(spec, &backend).await?;
    let dedup = load_dedup_table(&backend, &sealer).await?;
    let manifest = find_snapshot_by_id(&backend, &sealer, snapshot_id).await?;

    fs::create_dir_all(target_path).await?;
    let mut report = RunReport {
        mode: "restore".to_string(),
        snapshot_id: Some(manifest.id.clone()),
        ..Default::default()
    };

    // Cache of pack_id -> decrypted pack bytes. With encryption, ranged reads
    // don't help (AEAD is whole-message), so we just download the whole pack
    // once and reuse for every chunk it contains.
    let mut pack_cache: HashMap<String, bytes::Bytes> = HashMap::new();

    for file in &manifest.files {
        let dest = target_path.join(&file.path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await?;
        }
        let mut writer = fs::File::create(&dest).await?;
        let mut total = 0u64;

        for chunk_ref in &file.chunks {
            let entry = dedup.get(&chunk_ref.id).ok_or_else(|| {
                Error::Other(format!(
                    "chunk {} referenced by snapshot but missing from index",
                    chunk_ref.id
                ))
            })?;
            let chunk_bytes = fetch_chunk(&backend, &sealer, entry, &mut pack_cache).await?;
            if chunk_bytes.len() as u32 != chunk_ref.size {
                return Err(Error::Other(format!(
                    "chunk {} length mismatch (expected {}, got {})",
                    chunk_ref.id,
                    chunk_ref.size,
                    chunk_bytes.len()
                )));
            }
            writer.write_all(&chunk_bytes).await?;
            total += chunk_bytes.len() as u64;
        }
        writer.flush().await?;
        drop(writer);

        if let Some(mtime) = file.mtime {
            // Best-effort restore of mtime; ignore failures.
            let _ = set_mtime(&dest, mtime);
        }

        report.bytes_written += total;
        report.files_processed += 1;
    }

    tracing::info!(
        files = report.files_processed,
        bytes = report.bytes_written,
        snapshot_id = %manifest.id,
        "restore complete"
    );
    Ok(report)
}

async fn fetch_chunk(
    backend: &Arc<dyn StorageBackend>,
    sealer: &Sealer,
    entry: &PackIndexEntry,
    cache: &mut HashMap<String, bytes::Bytes>,
) -> Result<bytes::Bytes> {
    if !cache.contains_key(&entry.pack_id) {
        let key = repo_layout::pack_key(&entry.pack_id);
        let raw = backend.get(&key).await?;
        let opened = sealer.open(raw)?;
        cache.insert(entry.pack_id.clone(), opened);
        // Bound the cache so a giant restore doesn't OOM us. 256 packs ~=
        // 4 GiB at the default 16 MiB target — adjust if it bites.
        if cache.len() > 256 {
            // Drop the oldest-inserted entry. HashMap doesn't preserve order;
            // we just drain to half. Crude but effective for now.
            let drop_count = cache.len() / 2;
            let keys: Vec<String> = cache.keys().take(drop_count).cloned().collect();
            for k in keys {
                cache.remove(&k);
            }
        }
    }
    let pack = &cache[&entry.pack_id];
    let start = entry.offset as usize;
    let end = start + entry.length as usize;
    if end > pack.len() {
        return Err(Error::Other(format!(
            "chunk {} out of bounds in pack {}",
            entry.chunk_id, entry.pack_id
        )));
    }
    Ok(pack.slice(start..end))
}

async fn find_snapshot_by_id(
    backend: &Arc<dyn StorageBackend>,
    sealer: &Sealer,
    snapshot_id: &str,
) -> Result<SnapshotManifest> {
    let mut keys = Vec::new();
    {
        let mut stream = backend.list("snapshots/");
        while let Some(item) = stream.next().await {
            keys.push(item?.key);
        }
    }
    // Allow short ids: match by prefix.
    let needle = snapshot_id.to_lowercase();
    for key in keys {
        let raw = backend.get(&key).await?;
        let opened = sealer.open(raw)?;
        let manifest: SnapshotManifest = serde_json::from_slice(&opened)
            .map_err(|e| Error::Serde(format!("manifest parse: {e}")))?;
        if manifest.id == needle || manifest.id.starts_with(&needle) {
            return Ok(manifest);
        }
    }
    Err(Error::NotFound(format!("snapshot {snapshot_id}")))
}

async fn open_repo_sealer(
    spec: &BackupSpec,
    backend: &Arc<dyn StorageBackend>,
) -> Result<Sealer> {
    let raw = backend.get(REPO_CONFIG_KEY).await?;
    let config: RepoConfig = serde_json::from_slice(&raw)
        .map_err(|e| Error::Serde(format!("repo config: {e}")))?;
    match (config.encryption.as_ref(), spec.encryption.as_ref()) {
        (Some(header), Some(enc)) if enc.enabled => {
            let password = encryption::read_password_file(&enc.password_file)?;
            encryption::open_encryption(&password, header)
        }
        (Some(_), _) => Err(Error::Config(
            "repo is encrypted; restore needs spec.encryption.enabled with the original password"
                .into(),
        )),
        _ => Ok(Sealer::none()),
    }
}

async fn load_dedup_table(
    backend: &Arc<dyn StorageBackend>,
    sealer: &Sealer,
) -> Result<BTreeMap<String, PackIndexEntry>> {
    let mut table = BTreeMap::new();
    let mut keys = Vec::new();
    {
        let mut stream = backend.list("index/");
        while let Some(item) = stream.next().await {
            keys.push(item?.key);
        }
    }
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

fn set_mtime(path: &Path, secs: i64) -> std::io::Result<()> {
    let when = std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs.max(0) as u64);
    let times = std::fs::FileTimes::new().set_modified(when);
    std::fs::File::open(path)?.set_times(times)
}

