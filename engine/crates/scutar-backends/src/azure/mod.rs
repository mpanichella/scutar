//! Azure Blob Storage backend.
//!
//! Credentials: reads `AZURE_STORAGE_ACCOUNT` and `AZURE_STORAGE_KEY` from
//! the environment (the operator wires them from the Secret). Falls back to
//! the SAS token if `AZURE_STORAGE_SAS_TOKEN` is set.
//!
//! NOTE: the `azure_storage_blobs` crate's API surface has shifted between
//! versions. The implementation below targets the 0.21.x line. If a future
//! upgrade breaks compilation, the change is isolated to this single file.

use async_trait::async_trait;
use azure_storage::prelude::*;
use azure_storage_blobs::prelude::*;
use bytes::Bytes;
use futures::stream::{BoxStream, StreamExt};
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::sync::Arc;

pub struct AzureBackend {
    container: ContainerClient,
    prefix: String,
}

impl AzureBackend {
    fn key_for(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }
}

#[async_trait]
impl StorageBackend for AzureBackend {
    fn name(&self) -> &str {
        "azure"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_multipart: true,
            max_object_size: Some(190 * 1024 * 1024 * 1024 * 1024),
            supports_atomic_rename: false,
        }
    }

    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let full_key = self.key_for(key);
        let blob = self.container.blob_client(&full_key);
        blob.put_block_blob(data.to_vec())
            .await
            .map_err(|e| Error::Backend(format!("azure put_block_blob: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let blob = self.container.blob_client(&full_key);
        let mut stream = blob.get().into_stream();
        let mut buf = Vec::<u8>::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("BlobNotFound") || msg.contains("404") {
                    Error::NotFound(key.to_string())
                } else {
                    Error::Backend(format!("azure get: {e}"))
                }
            })?;
            let bytes = chunk
                .data
                .collect()
                .await
                .map_err(|e| Error::Backend(format!("azure body: {e}")))?;
            buf.extend_from_slice(&bytes);
        }
        Ok(Bytes::from(buf))
    }

    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let blob = self.container.blob_client(&full_key);
        let range = azure_core::request_options::Range::new(offset, offset + length);
        let mut stream = blob.get().range(range).into_stream();
        let mut buf = Vec::<u8>::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|e| Error::Backend(format!("azure get range: {e}")))?;
            let bytes = chunk
                .data
                .collect()
                .await
                .map_err(|e| Error::Backend(format!("azure body: {e}")))?;
            buf.extend_from_slice(&bytes);
        }
        Ok(Bytes::from(buf))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let full_key = self.key_for(key);
        let blob = self.container.blob_client(&full_key);
        match blob.get_properties().await {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("BlobNotFound") || msg.contains("404") {
                    Ok(false)
                } else {
                    Err(Error::Backend(format!("azure get_properties: {e}")))
                }
            }
        }
    }

    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> {
        let full_prefix = self.key_for(prefix);
        let strip_root = self.prefix.clone();
        let container = self.container.clone();
        Box::pin(async_stream::try_stream! {
            let mut stream = container.list_blobs().prefix(full_prefix).into_stream();
            while let Some(page) = stream.next().await {
                let page = page.map_err(|e| Error::Backend(format!("azure list: {e}")))?;
                for blob in page.blobs.blobs() {
                    let key = blob.name.clone();
                    let stripped = if !strip_root.is_empty() && key.starts_with(&strip_root) {
                        key[strip_root.len()..].trim_start_matches('/').to_string()
                    } else {
                        key
                    };
                    yield ObjectMeta {
                        key: stripped,
                        size: blob.properties.content_length,
                        last_modified: Some(blob.properties.last_modified.to_string()),
                        etag: Some(blob.properties.etag.to_string()),
                    };
                }
            }
        })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let full_key = self.key_for(key);
        let blob = self.container.blob_client(&full_key);
        match blob.delete().await {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("BlobNotFound") || msg.contains("404") {
                    Ok(())
                } else {
                    Err(Error::Backend(format!("azure delete: {e}")))
                }
            }
        }
    }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Azure {
            account,
            container,
            prefix,
        } => {
            let account_env = std::env::var("AZURE_STORAGE_ACCOUNT")
                .unwrap_or_else(|_| account.clone());
            let credentials = if let Ok(key) = std::env::var("AZURE_STORAGE_KEY") {
                StorageCredentials::access_key(account_env.clone(), key)
            } else if let Ok(sas) = std::env::var("AZURE_STORAGE_SAS_TOKEN") {
                StorageCredentials::sas_token(sas)
                    .map_err(|e| Error::Config(format!("invalid SAS token: {e}")))?
            } else {
                return Err(Error::Config(
                    "azure: set AZURE_STORAGE_KEY or AZURE_STORAGE_SAS_TOKEN".into(),
                ));
            };
            let blob_service = ClientBuilder::new(account_env, credentials).blob_service_client();
            let container_client = blob_service.container_client(container.clone());
            Ok(Arc::new(AzureBackend {
                container: container_client,
                prefix: prefix.clone().unwrap_or_default(),
            }))
        }
        _ => Err(Error::Config("expected Azure connection spec".into())),
    }
}

