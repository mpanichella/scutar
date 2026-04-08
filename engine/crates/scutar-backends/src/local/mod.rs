//! Local filesystem backend.
//!
//! Uses the local OS filesystem as the storage destination. Primarily intended
//! for tests, air-gapped clusters, and one-off backups to a shared volume
//! (e.g. an NFS PVC). Treats the configured `path` as the repo root and joins
//! object keys onto it with `/` as separator.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt};
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncSeekExt, SeekFrom};

pub struct LocalBackend {
    root: PathBuf,
}

impl LocalBackend {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    fn path_for(&self, key: &str) -> PathBuf {
        // Defensive: reject absolute keys and `..` traversal so a buggy caller
        // can't escape the repo root.
        let cleaned: PathBuf = Path::new(key)
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => Some(s),
                _ => None,
            })
            .collect();
        self.root.join(cleaned)
    }
}

#[async_trait]
impl StorageBackend for LocalBackend {
    fn name(&self) -> &str {
        "local"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_multipart: false,
            max_object_size: None,
            supports_atomic_rename: true,
        }
    }

    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let path = self.path_for(key);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        // Atomic write: tmp file in same dir + rename.
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        fs::write(&tmp, &data[..]).await?;
        fs::rename(&tmp, &path).await?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let path = self.path_for(key);
        match fs::read(&path).await {
            Ok(v) => Ok(Bytes::from(v)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(Error::NotFound(key.to_string()))
            }
            Err(e) => Err(Error::Io(e)),
        }
    }

    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> {
        let path = self.path_for(key);
        let mut f = match fs::File::open(&path).await {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(Error::NotFound(key.to_string()))
            }
            Err(e) => return Err(Error::Io(e)),
        };
        f.seek(SeekFrom::Start(offset)).await?;
        let mut buf = vec![0u8; length as usize];
        let mut read = 0usize;
        while read < buf.len() {
            let n = f.read(&mut buf[read..]).await?;
            if n == 0 {
                buf.truncate(read);
                break;
            }
            read += n;
        }
        Ok(Bytes::from(buf))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        Ok(fs::try_exists(self.path_for(key)).await?)
    }

    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> {
        let root = self.root.clone();
        let prefix = prefix.to_string();
        // Walk eagerly into a Vec — local FS lists are cheap and avoiding the
        // streaming complexity here keeps the impl small.
        let fut = async move {
            let base = if prefix.is_empty() {
                root.clone()
            } else {
                root.join(&prefix)
            };
            let mut out: Vec<Result<ObjectMeta>> = Vec::new();
            let mut stack = vec![base.clone()];
            while let Some(dir) = stack.pop() {
                let mut rd = match fs::read_dir(&dir).await {
                    Ok(rd) => rd,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(e) => {
                        out.push(Err(Error::Io(e)));
                        continue;
                    }
                };
                loop {
                    match rd.next_entry().await {
                        Ok(Some(entry)) => {
                            let path = entry.path();
                            let meta = match entry.metadata().await {
                                Ok(m) => m,
                                Err(e) => {
                                    out.push(Err(Error::Io(e)));
                                    continue;
                                }
                            };
                            if meta.is_dir() {
                                stack.push(path);
                            } else if meta.is_file() {
                                let rel = path
                                    .strip_prefix(&root)
                                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                                    .unwrap_or_default();
                                out.push(Ok(ObjectMeta {
                                    key: rel,
                                    size: meta.len(),
                                    last_modified: meta
                                        .modified()
                                        .ok()
                                        .and_then(|t| {
                                            t.duration_since(std::time::UNIX_EPOCH).ok()
                                        })
                                        .map(|d| format!("epoch:{}", d.as_secs())),
                                    etag: None,
                                }));
                            }
                        }
                        Ok(None) => break,
                        Err(e) => {
                            out.push(Err(Error::Io(e)));
                            break;
                        }
                    }
                }
            }
            out
        };
        Box::pin(stream::once(fut).flat_map(|v| stream::iter(v.into_iter())))
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.path_for(key);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()), // idempotent
            Err(e) => Err(Error::Io(e)),
        }
    }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Local { path } => {
            tokio::fs::create_dir_all(path).await?;
            Ok(Arc::new(LocalBackend::new(path.clone())))
        }
        _ => Err(Error::Config("expected Local connection spec".into())),
    }
}
