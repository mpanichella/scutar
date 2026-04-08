//! Google Cloud Storage backend.
//!
//! Credentials: reads `GOOGLE_APPLICATION_CREDENTIALS` (a path to a service
//! account JSON file). The operator mounts the Secret as a file under
//! `/etc/scutar/creds/service-account.json` and exports the env var.
//!
//! NOTE: the `google-cloud-storage` crate's API has shifted across versions.
//! This implementation targets the 0.22.x line. If a future upgrade breaks
//! compilation, the change is contained to this single file.

use async_trait::async_trait;
use bytes::Bytes;
use futures::stream::BoxStream;
use google_cloud_storage::client::{Client, ClientConfig};
use google_cloud_storage::http::objects::delete::DeleteObjectRequest;
use google_cloud_storage::http::objects::download::Range;
use google_cloud_storage::http::objects::get::GetObjectRequest;
use google_cloud_storage::http::objects::list::ListObjectsRequest;
use google_cloud_storage::http::objects::upload::{Media, UploadObjectRequest, UploadType};
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::sync::Arc;

pub struct GcsBackend {
    client: Client,
    bucket: String,
    prefix: String,
}

impl GcsBackend {
    fn key_for(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }
}

#[async_trait]
impl StorageBackend for GcsBackend {
    fn name(&self) -> &str {
        "gcs"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_multipart: true,
            max_object_size: Some(5 * 1024 * 1024 * 1024 * 1024),
            supports_atomic_rename: false,
        }
    }

    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let full_key = self.key_for(key);
        let media = Media::new(full_key);
        self.client
            .upload_object(
                &UploadObjectRequest {
                    bucket: self.bucket.clone(),
                    ..Default::default()
                },
                data.to_vec(),
                &UploadType::Simple(media),
            )
            .await
            .map_err(|e| Error::Backend(format!("gcs upload: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let bytes = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: full_key,
                    ..Default::default()
                },
                &Range::default(),
            )
            .await
            .map_err(|e| {
                let msg = format!("{e}");
                if msg.contains("404") || msg.contains("notFound") {
                    Error::NotFound(key.to_string())
                } else {
                    Error::Backend(format!("gcs download: {e}"))
                }
            })?;
        Ok(Bytes::from(bytes))
    }

    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let bytes = self
            .client
            .download_object(
                &GetObjectRequest {
                    bucket: self.bucket.clone(),
                    object: full_key,
                    ..Default::default()
                },
                &Range(Some(offset), Some(offset + length - 1)),
            )
            .await
            .map_err(|e| Error::Backend(format!("gcs download range: {e}")))?;
        Ok(Bytes::from(bytes))
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let full_key = self.key_for(key);
        match self
            .client
            .get_object(&GetObjectRequest {
                bucket: self.bucket.clone(),
                object: full_key,
                ..Default::default()
            })
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("404") || msg.contains("notFound") {
                    Ok(false)
                } else {
                    Err(Error::Backend(format!("gcs get_object: {e}")))
                }
            }
        }
    }

    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> {
        let full_prefix = self.key_for(prefix);
        let strip_root = self.prefix.clone();
        let bucket = self.bucket.clone();
        let client = self.client.clone();
        Box::pin(async_stream::try_stream! {
            let mut page_token: Option<String> = None;
            loop {
                let req = ListObjectsRequest {
                    bucket: bucket.clone(),
                    prefix: Some(full_prefix.clone()),
                    page_token: page_token.clone(),
                    ..Default::default()
                };
                let res = client
                    .list_objects(&req)
                    .await
                    .map_err(|e| Error::Backend(format!("gcs list: {e}")))?;
                if let Some(items) = res.items {
                    for obj in items {
                        let key = obj.name;
                        let stripped = if !strip_root.is_empty() && key.starts_with(&strip_root) {
                            key[strip_root.len()..].trim_start_matches('/').to_string()
                        } else {
                            key
                        };
                        yield ObjectMeta {
                            key: stripped,
                            size: obj.size as u64,
                            last_modified: obj.updated.map(|t| t.to_string()),
                            etag: Some(obj.etag),
                        };
                    }
                }
                match res.next_page_token {
                    Some(t) if !t.is_empty() => page_token = Some(t),
                    _ => break,
                }
            }
        })
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let full_key = self.key_for(key);
        match self
            .client
            .delete_object(&DeleteObjectRequest {
                bucket: self.bucket.clone(),
                object: full_key,
                ..Default::default()
            })
            .await
        {
            Ok(_) => Ok(()),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("404") || msg.contains("notFound") {
                    Ok(())
                } else {
                    Err(Error::Backend(format!("gcs delete: {e}")))
                }
            }
        }
    }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::Gcs { bucket, prefix } => {
            let config = ClientConfig::default()
                .with_auth()
                .await
                .map_err(|e| Error::Config(format!("gcs auth: {e}")))?;
            let client = Client::new(config);
            Ok(Arc::new(GcsBackend {
                client,
                bucket: bucket.clone(),
                prefix: prefix.clone().unwrap_or_default(),
            }))
        }
        _ => Err(Error::Config("expected GCS connection spec".into())),
    }
}
