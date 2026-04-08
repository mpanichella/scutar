//! On-disk / on-bucket layout of a Scutar snapshot repository.
//!
//! All keys are joined with `/` (POSIX), regardless of the backend. Backends
//! are responsible for adapting paths to their native format if needed
//! (e.g. SFTP uses real filesystem separators).
//!
//! ```text
//! <repo-root>/
//!   config.json                  ← repo metadata (version, dedup params, encryption header)
//!   index/
//!     <index-id>.json             ← chunk → pack lookup, sharded by id prefix
//!   data/
//!     <pack-id>.pack              ← concatenated chunks (optionally encrypted)
//!   snapshots/
//!     <iso8601>-<short-id>.json   ← snapshot manifest (tree + chunk refs)
//!   locks/
//!     <lock-id>                    ← optional advisory lock
//! ```
//!
//! For `mirror` mode the layout is much simpler — files are written under the
//! repo root with their original relative paths, plus a `.scutar-mirror-state.json`
//! file at the root tracking BLAKE3 hashes for change detection.

use std::fmt::Write as _;

pub const REPO_CONFIG_KEY: &str = "config.json";
pub const MIRROR_STATE_KEY: &str = ".scutar-mirror-state.json";

pub fn snapshot_key(timestamp_iso: &str, short_id: &str) -> String {
    format!("snapshots/{timestamp_iso}-{short_id}.json")
}

pub fn pack_key(pack_id: &str) -> String {
    format!("data/{pack_id}.pack")
}

pub fn index_key(index_id: &str) -> String {
    format!("index/{index_id}.json")
}

/// Encode a binary id (chunk hash, pack hash, ...) as lowercase hex.
pub fn hex_id(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}

/// Short, human-readable id (first 8 hex chars). Used for snapshot keys
/// alongside the timestamp; the full id lives in the manifest.
pub fn short_id(full_hex: &str) -> &str {
    &full_hex[..full_hex.len().min(12)]
}
