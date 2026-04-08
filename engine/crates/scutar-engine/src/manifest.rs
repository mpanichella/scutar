//! Snapshot manifest format.
//!
//! A snapshot is a pair of:
//!   * A `SnapshotManifest` (this file) — JSON, serialized to
//!     `snapshots/<timestamp>-<short-id>.json`. Lists every file in the source
//!     and the chunks that compose it.
//!   * Pack files containing the actual chunk data, stored under `data/`.
//!
//! Pack file index entries map `chunk_id -> (pack_id, offset, length)`. They
//! live in `index/<index-id>.json`. The engine loads all index files at
//! startup to build an in-memory dedup table.
//!
//! Repository config (`config.json`) describes version + crypto parameters
//! and is read first by both backup and restore.

use serde::{Deserialize, Serialize};

pub const REPO_FORMAT_VERSION: u32 = 1;
pub const REPO_MAGIC: &str = "scutar-repo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub magic: String,
    pub version: u32,
    /// Average target chunk size in bytes (FastCDC parameter).
    pub avg_chunk_size: u32,
    /// Min/max chunk size in bytes.
    pub min_chunk_size: u32,
    pub max_chunk_size: u32,
    /// Pack target size: chunks are accumulated into a pack until it reaches
    /// roughly this size, then the pack is flushed.
    pub pack_target_size: u64,
    /// Encryption header. `None` means the repo is unencrypted.
    pub encryption: Option<EncryptionHeader>,
}

impl RepoConfig {
    pub fn new_default() -> Self {
        Self {
            magic: REPO_MAGIC.to_string(),
            version: REPO_FORMAT_VERSION,
            avg_chunk_size: 1024 * 1024,        // 1 MiB
            min_chunk_size: 256 * 1024,         // 256 KiB
            max_chunk_size: 8 * 1024 * 1024,    // 8 MiB
            pack_target_size: 16 * 1024 * 1024, // 16 MiB
            encryption: None,
        }
    }
}

/// Stored in `config.json` when the repo is encrypted. The data key (random
/// 32 bytes) is wrapped with a key-encryption-key derived from the user
/// password via Argon2id. To decrypt, the engine derives the KEK with the
/// same parameters and unwraps the data key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionHeader {
    pub kdf: String, // "argon2id"
    pub kdf_salt_b64: String,
    pub kdf_m_cost: u32,
    pub kdf_t_cost: u32,
    pub kdf_p_cost: u32,
    /// Wrapped (encrypted) data key. AES-256-GCM.
    pub wrapped_data_key_b64: String,
    pub wrap_nonce_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotManifest {
    /// BLAKE3 of the manifest contents (filled in after serialization).
    pub id: String,
    pub created_at: String, // RFC3339
    pub backup_name: String,
    pub source_root: String,
    pub bytes_total: u64,
    pub files: Vec<FileEntry>,
    #[serde(default)]
    pub labels: std::collections::BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub size: u64,
    pub mtime: Option<i64>,
    pub mode: Option<u32>,
    pub chunks: Vec<ChunkRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkRef {
    /// Hex BLAKE3 of the chunk's plaintext.
    pub id: String,
    pub size: u32,
}

/// Index entry that maps a chunk id to its location inside a pack file.
/// Multiple chunks live in the same pack; the offset/length identify a
/// contiguous slice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackIndexEntry {
    pub chunk_id: String,
    pub pack_id: String,
    pub offset: u64,
    pub length: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PackIndex {
    pub entries: Vec<PackIndexEntry>,
}
