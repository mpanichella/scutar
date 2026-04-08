//! Pack file builder.
//!
//! A pack is a flat concatenation of chunks. We avoid uploading every chunk
//! as its own object because most clouds charge per-request and small-object
//! uploads are slow. Instead, chunks accumulate in a buffer until it crosses
//! `pack_target_size`, then the buffer is sealed (optionally encrypted) and
//! uploaded as a single object under `data/<pack-id>.pack`.
//!
//! Each pack ships an in-memory `PackIndex` listing `(chunk_id, offset, length)`
//! so the engine can later fetch a single chunk via `get_range`.
//!
//! Pack id = BLAKE3 of the (post-encryption, if any) pack bytes.

use bytes::{BufMut, Bytes, BytesMut};
use scutar_core::{repo_layout, Result, StorageBackend};
use std::sync::Arc;

use crate::chunker::Chunk;
use crate::encryption::Sealer;
use crate::manifest::{PackIndex, PackIndexEntry};

pub struct Packer {
    target_size: u64,
    buf: BytesMut,
    /// Per-chunk index entries pending the next flush. We track logical
    /// (plaintext) offset/length here so consumers can fetch individual chunks
    /// via byte ranges over the (encrypted) pack — note that with encryption
    /// enabled, ranged reads need to fetch the *whole* pack and decrypt it,
    /// because AEAD is whole-message. The unencrypted path supports true
    /// range reads.
    pending: Vec<PendingChunk>,
    pub index: PackIndex,
    pub bytes_written: u64,
    pub packs_uploaded: u64,
}

struct PendingChunk {
    id: String,
    offset: u64,
    length: u32,
}

impl Packer {
    pub fn new(target_size: u64) -> Self {
        Self {
            target_size,
            buf: BytesMut::with_capacity(target_size as usize),
            pending: Vec::new(),
            index: PackIndex::default(),
            bytes_written: 0,
            packs_uploaded: 0,
        }
    }

    /// Append a chunk to the current pack. Returns `Some(())` if the pack
    /// reached the target size and should be flushed via `flush`.
    pub fn add(&mut self, chunk: &Chunk) -> bool {
        let offset = self.buf.len() as u64;
        self.buf.put_slice(&chunk.data);
        self.pending.push(PendingChunk {
            id: chunk.id.clone(),
            offset,
            length: chunk.data.len() as u32,
        });
        self.buf.len() as u64 >= self.target_size
    }

    /// Seal and upload the current pack (if non-empty). Records index entries
    /// against the resulting `pack_id`.
    pub async fn flush(
        &mut self,
        backend: &Arc<dyn StorageBackend>,
        sealer: &Sealer,
    ) -> Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let plaintext = self.buf.split().freeze();
        let plaintext_len = plaintext.len();
        let sealed = sealer.seal(plaintext)?;
        let pack_id = blake3_hex(&sealed);
        let key = repo_layout::pack_key(&pack_id);
        backend.put(&key, sealed.clone()).await?;
        self.bytes_written += sealed.len() as u64;
        self.packs_uploaded += 1;

        for pc in self.pending.drain(..) {
            self.index.entries.push(PackIndexEntry {
                chunk_id: pc.id,
                pack_id: pack_id.clone(),
                offset: pc.offset,
                length: pc.length,
            });
        }

        tracing::debug!(
            pack_id = %pack_id,
            plaintext_len,
            sealed_len = sealed.len(),
            "packer: flushed pack"
        );
        Ok(())
    }
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
