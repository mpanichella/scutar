//! Amazon S3 backend (and S3-compatible: MinIO, Wasabi, DigitalOcean Spaces).
//!
//! Credential discovery follows the standard AWS SDK chain:
//!   * `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`
//!   * `~/.aws/credentials`
//!   * IRSA / instance profile
//!
//! The operator wires the env vars from the `ScutarConnection.credentialsSecretRef`
//! Secret, so the SDK picks them up automatically. Nothing here parses
//! credentials by hand.

use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_s3::config::Region;
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart};
use aws_sdk_s3::Client;
use bytes::Bytes;
use futures::stream::{self, BoxStream, StreamExt};
use scutar_core::{
    BackendCapabilities, ConnectionSpec, Error, ObjectMeta, Result, StorageBackend,
};
use std::sync::Arc;

/// Threshold above which we switch from a single PUT to multipart upload.
const MULTIPART_THRESHOLD: usize = 16 * 1024 * 1024; // 16 MiB
/// Size of each multipart chunk (must be >= 5 MiB except for the last part).
const MULTIPART_CHUNK_SIZE: usize = 8 * 1024 * 1024; // 8 MiB

pub struct S3Backend {
    client: Client,
    bucket: String,
    prefix: String,
}

impl S3Backend {
    fn key_for(&self, key: &str) -> String {
        if self.prefix.is_empty() {
            key.to_string()
        } else {
            format!("{}/{}", self.prefix.trim_end_matches('/'), key)
        }
    }
}

#[async_trait]
impl StorageBackend for S3Backend {
    fn name(&self) -> &str {
        "s3"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_multipart: true,
            max_object_size: Some(5 * 1024 * 1024 * 1024 * 1024), // 5 TiB
            supports_atomic_rename: false,
        }
    }

    async fn put(&self, key: &str, data: Bytes) -> Result<()> {
        let full_key = self.key_for(key);
        if data.len() <= MULTIPART_THRESHOLD {
            self.client
                .put_object()
                .bucket(&self.bucket)
                .key(&full_key)
                .body(ByteStream::from(data))
                .send()
                .await
                .map_err(|e| Error::Backend(format!("s3 put_object: {e}")))?;
            return Ok(());
        }

        // ---- multipart ----
        let create = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| Error::Backend(format!("s3 create_multipart: {e}")))?;
        let upload_id = create
            .upload_id()
            .ok_or_else(|| Error::Backend("s3 create_multipart: missing upload_id".into()))?
            .to_string();

        let mut completed = Vec::<CompletedPart>::new();
        let mut part_number = 1i32;
        let mut offset = 0usize;
        while offset < data.len() {
            let end = (offset + MULTIPART_CHUNK_SIZE).min(data.len());
            let part = data.slice(offset..end);
            let res = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(&full_key)
                .upload_id(&upload_id)
                .part_number(part_number)
                .body(ByteStream::from(part))
                .send()
                .await;
            match res {
                Ok(out) => {
                    completed.push(
                        CompletedPart::builder()
                            .set_e_tag(out.e_tag)
                            .part_number(part_number)
                            .build(),
                    );
                }
                Err(e) => {
                    let _ = self
                        .client
                        .abort_multipart_upload()
                        .bucket(&self.bucket)
                        .key(&full_key)
                        .upload_id(&upload_id)
                        .send()
                        .await;
                    return Err(Error::Backend(format!("s3 upload_part: {e}")));
                }
            }
            part_number += 1;
            offset = end;
        }

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(&full_key)
            .upload_id(&upload_id)
            .multipart_upload(
                CompletedMultipartUpload::builder()
                    .set_parts(Some(completed))
                    .build(),
            )
            .send()
            .await
            .map_err(|e| Error::Backend(format!("s3 complete_multipart: {e}")))?;
        Ok(())
    }

    async fn get(&self, key: &str) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| {
                if format!("{e}").contains("NoSuchKey") {
                    Error::NotFound(key.to_string())
                } else {
                    Error::Backend(format!("s3 get_object: {e}"))
                }
            })?;
        let bytes = out
            .body
            .collect()
            .await
            .map_err(|e| Error::Backend(format!("s3 body collect: {e}")))?
            .into_bytes();
        Ok(bytes)
    }

    async fn get_range(&self, key: &str, offset: u64, length: u64) -> Result<Bytes> {
        let full_key = self.key_for(key);
        let range = format!("bytes={}-{}", offset, offset + length - 1);
        let out = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .range(range)
            .send()
            .await
            .map_err(|e| {
                if format!("{e}").contains("NoSuchKey") {
                    Error::NotFound(key.to_string())
                } else {
                    Error::Backend(format!("s3 get_object range: {e}"))
                }
            })?;
        Ok(out
            .body
            .collect()
            .await
            .map_err(|e| Error::Backend(format!("s3 body collect: {e}")))?
            .into_bytes())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let full_key = self.key_for(key);
        match self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
        {
            Ok(_) => Ok(true),
            Err(e) => {
                let msg = format!("{e}");
                if msg.contains("NotFound") || msg.contains("404") {
                    Ok(false)
                } else {
                    Err(Error::Backend(format!("s3 head_object: {e}")))
                }
            }
        }
    }

    fn list<'a>(&'a self, prefix: &'a str) -> BoxStream<'a, Result<ObjectMeta>> {
        let bucket = self.bucket.clone();
        let full_prefix = self.key_for(prefix);
        let client = self.client.clone();
        let strip_root = self.prefix.clone();
        Box::pin(stream::unfold(
            (Some(String::new()), client, bucket, full_prefix, strip_root),
            move |(token, client, bucket, full_prefix, strip_root)| async move {
                let token = token?;
                let mut req = client
                    .list_objects_v2()
                    .bucket(&bucket)
                    .prefix(&full_prefix);
                if !token.is_empty() {
                    req = req.continuation_token(token);
                }
                match req.send().await {
                    Ok(out) => {
                        let next = if out.is_truncated().unwrap_or(false) {
                            out.next_continuation_token().map(|s| s.to_string())
                        } else {
                            None
                        };
                        let items: Vec<Result<ObjectMeta>> = out
                            .contents()
                            .iter()
                            .map(|obj| {
                                let key = obj.key().unwrap_or_default().to_string();
                                let stripped = if !strip_root.is_empty()
                                    && key.starts_with(&strip_root)
                                {
                                    key[strip_root.len()..]
                                        .trim_start_matches('/')
                                        .to_string()
                                } else {
                                    key
                                };
                                Ok(ObjectMeta {
                                    key: stripped,
                                    size: obj.size().unwrap_or(0) as u64,
                                    last_modified: obj
                                        .last_modified()
                                        .map(|t| t.to_string()),
                                    etag: obj.e_tag().map(|s| s.to_string()),
                                })
                            })
                            .collect();
                        Some((
                            stream::iter(items),
                            (next, client, bucket, full_prefix, strip_root),
                        ))
                    }
                    Err(e) => Some((
                        stream::iter(vec![Err(Error::Backend(format!("s3 list: {e}")))]),
                        (None, client, bucket, full_prefix, strip_root),
                    )),
                }
            },
        )
        .flatten())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        let full_key = self.key_for(key);
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&full_key)
            .send()
            .await
            .map_err(|e| Error::Backend(format!("s3 delete_object: {e}")))?;
        Ok(())
    }
}

pub async fn build(spec: &ConnectionSpec) -> Result<Arc<dyn StorageBackend>> {
    match spec {
        ConnectionSpec::S3 {
            bucket,
            region,
            prefix,
            endpoint,
            force_path_style,
        } => {
            let mut loader = aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(region.clone()));
            if let Some(ep) = endpoint {
                loader = loader.endpoint_url(ep);
            }
            let shared = loader.load().await;
            let mut s3_conf = aws_sdk_s3::config::Builder::from(&shared);
            if *force_path_style {
                s3_conf = s3_conf.force_path_style(true);
            }
            let client = Client::from_conf(s3_conf.build());
            Ok(Arc::new(S3Backend {
                client,
                bucket: bucket.clone(),
                prefix: prefix.clone().unwrap_or_default(),
            }))
        }
        _ => Err(Error::Config("expected S3 connection spec".into())),
    }
}
