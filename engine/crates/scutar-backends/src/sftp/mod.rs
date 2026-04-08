//! SFTP backend over SSH (russh + russh-sftp).
//!
//! Credentials: reads from `credentials_dir` (mounted by the operator):
//!   * `id_ed25519` or `id_rsa` — private key (preferred)
//!   * `password` — password file (fallback)
//!   * `passphrase` — optional passphrase for an encrypted private key
//!
//! NOTE: this backend opens a single SSH connection and serializes operations
//! through a Mutex. SFTP libraries are typically not Send across .await points,
//! so the Mutex keeps things safe and simple. For high concurrency we'd want a
//! pool of connections — easy follow-up.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use russh::client::{self, Handle};
use russh::keys::{decode_secret_key, key};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt, SeekFrom};
use tokio::sync::Mutex;

pub struct SftpBackend {
    /// Mutex around the live SFTP session — see module docs.
    session: Mutex<SftpSession>,
    /// Kept alive so the underlying SSH connection isn't dropped.
    _ssh: Handle<SftpClientHandler>,
    root: PathBuf,
}

struct SftpClientHandler;

#[async_trait]
impl client::Handler for SftpClientHandler {
    type Error = russh::Error;
    async fn check_server_key(
        &mut self,
        _server_public_key: &key::PublicKey,
    ) -> std::result::Result<bool, Self::Error> {
        // TODO(security): pin the server's host key via a known_hosts file
        // mounted by the operator. Accepting any key here is fine for the
        // initial implementation but must be tightened before production use.
        Ok(true)
    }
}

impl SftpBackend {
    fn path_for(&self, key: &str) -> PathBuf {
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
impl StorageBackend for SftpBackend {
    fn name(&self) -> &str {
        "sftp"
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
        let path_str = path.to_string_lossy().to_string();
        let session = self.session.lock().await;

        // Ensure parent directory exists (mkdir -p equivalent).
        if let Some(parent) = path.parent() {
            let mut acc = PathBuf::new();
            for comp in parent.components() {
                if let std::path::Component::Normal(p) = comp {
                    acc.push(p);
                    let abs = if parent.is_absolute() {
                        Path::new("/").join(&acc)
                    } else {
                        acc.clone()
                    };
                    let _ = session.create_dir(abs.to_string_lossy().to_string()).await;
                }
            }
        }

        let mut file = session
            .open_with_flags(
                path_str,
                OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
            )
            .await
            .map_err(|e| Error::Backend(format!("sftp open: {e}")))?;
        file.write_all(&data)
            .await
            .map_err(|e| Error::Backend(format!("sftp write: {e}")))?;
        file.shutdown()
            .await
            .map_err(|e| Error::Backend(format!("sftp close: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let path = self.path_for(key);
        let path_str = path.to_string_lossy().to_string();
        let session = self.session.lock().await;
        let mut file = session
            .open(path_str)
            .await
            .map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("NoSuchFile") || msg.contains("No such") {
                    Error::NotFound(key.to_string())
                } else {
                    Error::Backend(format!("sftp open: {e}"))
                }
            })?;
        let mut buf = Vec::<u8>::new();
        file.read_to_end(&mut buf)
            .await
            .map_err(|e| Error::Backend(format!("sftp read: {e}")))?;
        Ok(Bytes::from(buf))
    }

    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> {
        let path = self.path_for(key);
        let path_str = path.to_string_lossy().to_string();
        let session = self.session.lock().await;
        let mut file = session
            .open(path_str)
            .await
            .map_err(|e| Error::Backend(format!("sftp open: {e}")))?;
        file.seek(SeekFrom::Start(offset))
            .await
            .map_err(|e| Error::Backend(format!("sftp seek: {e}")))?;
        let mut buf = vec![0u8; length as usize];
        let mut read = 0usize;
        while read < buf.len() {
            let n = file
                .read(&mut buf[read..])
                .await
                .map_err(|e| Error::Backend(format!("sftp read: {e}")))?;
            if n == 0 {
                buf.truncate(read);
                break;
            }
            read += n;
        }
        Ok(Bytes::from(buf))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let path = self.path_for(key);
        let session = self.session.lock().await;
        Ok(session
            .metadata(path.to_string_lossy().to_string())
            .await
            .is_ok())
    }

    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> {
        let base = self.path_for(prefix);
        let root = self.root.clone();
        let session_mtx = &self.session;
        Box::pin(async_stream::try_stream! {
            let session = session_mtx.lock().await;
            let mut stack = vec![base];
            while let Some(dir) = stack.pop() {
                let dir_str = dir.to_string_lossy().to_string();
                let entries = match session.read_dir(dir_str).await {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                for entry in entries {
                    let name = entry.file_name();
                    if name == "." || name == ".." {
                        continue;
                    }
                    let path = dir.join(&name);
                    let meta = entry.metadata();
                    if meta.is_dir() {
                        stack.push(path);
                    } else if meta.is_regular() {
                        let rel = path
                            .strip_prefix(&root)
                            .map(|p| p.to_string_lossy().replace('\\', "/"))
                            .unwrap_or_default();
                        yield ObjectMeta {
                            key: rel,
                            size: meta.size.unwrap_or(0),
                            last_modified: meta.mtime.map(|t| format!("epoch:{t}")),
                            etag: None,
                        };
                    }
                }
            }
        })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let path = self.path_for(key);
        let session = self.session.lock().await;
        match session.remove_file(path.to_string_lossy().to_string()).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("NoSuchFile") || msg.contains("No such") {
                    Ok(())
                } else {
                    Err(Error::Backend(format!("sftp delete: {e}")))
                }
            }
        }
    }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Sftp {
            host,
            port,
            user,
            path,
        } => {
            let addr = format!("{}:{}", host, port.unwrap_or(22));
            let config = Arc::new(client::Config::default());
            let mut handle = client::connect(config, addr, SftpClientHandler)
                .await
                .map_err(|e| Error::Backend(format!("ssh connect: {e}")))?;

            // Authentication: try private key first, then password.
            let creds_dir = std::env::var("SCUTAR_CREDENTIALS_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("/etc/scutar/creds"));

            let authed = try_pubkey_auth(&mut handle, user, &creds_dir).await?
                || try_password_auth(&mut handle, user, &creds_dir).await?;

            if !authed {
                return Err(Error::Config(
                    "sftp: no usable credentials in credentials_dir (expected id_ed25519, id_rsa or password)".into(),
                ));
            }

            let channel = handle
                .channel_open_session()
                .await
                .map_err(|e| Error::Backend(format!("ssh open channel: {e}")))?;
            channel
                .request_subsystem(true, "sftp")
                .await
                .map_err(|e| Error::Backend(format!("ssh subsystem sftp: {e}")))?;
            let session = SftpSession::new(channel.into_stream())
                .await
                .map_err(|e| Error::Backend(format!("sftp init: {e}")))?;

            Ok(Arc::new(SftpBackend {
                session: Mutex::new(session),
                _ssh: handle,
                root: path.clone(),
            }))
        }
        _ => Err(Error::Config("expected SFTP connection spec".into())),
    }
}

async fn try_pubkey_auth(
    handle: &mut Handle<SftpClientHandler>,
    user: &str,
    creds_dir: &Path,
) -> Result<bool> {
    for filename in ["id_ed25519", "id_rsa", "id_ecdsa"] {
        let key_path = creds_dir.join(filename);
        if !key_path.exists() {
            continue;
        }
        let bytes = tokio::fs::read(&key_path).await?;
        let passphrase = tokio::fs::read_to_string(creds_dir.join("passphrase"))
            .await
            .ok()
            .map(|s| s.trim().to_string());
        let key = decode_secret_key(
            std::str::from_utf8(&bytes)
                .map_err(|e| Error::Config(format!("ssh key not utf-8: {e}")))?,
            passphrase.as_deref(),
        )
        .map_err(|e| Error::Config(format!("decode ssh key: {e}")))?;
        let ok = handle
            .authenticate_publickey(user, Arc::new(key))
            .await
            .map_err(|e| Error::Backend(format!("ssh pubkey auth: {e}")))?;
        if ok {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn try_password_auth(
    handle: &mut Handle<SftpClientHandler>,
    user: &str,
    creds_dir: &Path,
) -> Result<bool> {
    let pwd_path = creds_dir.join("password");
    if !pwd_path.exists() {
        return Ok(false);
    }
    let password = tokio::fs::read_to_string(&pwd_path).await?;
    let ok = handle
        .authenticate_password(user, password.trim())
        .await
        .map_err(|e| Error::Backend(format!("ssh password auth: {e}")))?;
    Ok(ok)
}
