//! Content-defined chunking via FastCDC.
//!
//! Splits a file's bytes into variable-sized chunks at content-defined
//! boundaries. The boundaries depend only on the bytes themselves, so
//! inserting/removing data only changes the chunks at the edit point —
//! everything else stays identical and gets deduplicated for free.
//!
//! Each chunk is identified by BLAKE3 of its plaintext.

use bytes::Bytes;
use fastcdc::v2020::FastCDC;

#[derive(Debug, Clone)]
pub struct Chunk {
    /// Hex BLAKE3 id of `data`.
    pub id: String,
    pub data: Bytes,
}

pub struct ChunkerParams {
    pub min_size: u32,
    pub avg_size: u32,
    pub max_size: u32,
}

impl ChunkerParams {
    pub fn from_repo_config(c: &crate::manifest::RepoConfig) -> Self {
        Self {
            min_size: c.min_chunk_size,
            avg_size: c.avg_chunk_size,
            max_size: c.max_chunk_size,
        }
    }
}

/// Split a buffer into chunks. Returns owned `Bytes` slices that point into
/// the original buffer (cheap clones).
pub fn chunk_bytes(data: Bytes, params: &ChunkerParams) -> Vec<Chunk> {
    let mut out = Vec::new();
    let cdc = FastCDC::new(&data, params.min_size, params.avg_size, params.max_size);
    for entry in cdc {
        let slice = data.slice(entry.offset..entry.offset + entry.length);
        let hash = blake3::hash(&slice);
        out.push(Chunk {
            id: hex(hash.as_bytes()),
            data: slice,
        });
    }
    out
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut s, "{:02x}", b);
    }
    s
}
