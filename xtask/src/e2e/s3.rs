//! MinIO/S3 access for the kafka-to-s3 suite: creates a per-run bucket,
//! polls it for archived objects, reads them back and deletes the bucket
//! afterwards.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, bail};
use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::{Delete, ObjectIdentifier};
use tokio::time::Instant;

/// Endpoint used when `S3_ENDPOINT` is unset: the compose MinIO.
const DEFAULT_ENDPOINT: &str = "http://localhost:9000";
/// Credentials used when `S3_ACCESS_KEY`/`S3_SECRET_KEY` are unset: the
/// compose MinIO's root user.
const DEFAULT_ACCESS_KEY: &str = "minioadmin";
const DEFAULT_SECRET_KEY: &str = "minioadmin";
/// MinIO ignores the region, but the SDK and the archiver need one.
pub(crate) const REGION: &str = "us-east-1";
const POLL_INTERVAL: Duration = Duration::from_millis(500);

/// An archived object read back from the bucket.
pub(crate) struct Object {
    /// The uncompressed JSONL body.
    pub(crate) body: Vec<u8>,
    /// User metadata, keyed by name without the `x-amz-meta-` prefix.
    pub(crate) metadata: HashMap<String, String>,
}

impl Object {
    /// The body's lines, without their trailing newlines.
    pub(crate) fn lines(&self) -> anyhow::Result<Vec<&str>> {
        let text = std::str::from_utf8(&self.body).context("object body is not UTF-8")?;
        Ok(text.lines().collect())
    }
}

pub(crate) struct S3 {
    client: Client,
    endpoint: String,
    access_key: String,
    secret_key: String,
    bucket: String,
}

impl S3 {
    /// Connects to `S3_ENDPOINT` (default: the compose MinIO), checks it
    /// answers, and creates the per-run `bucket`.
    pub(crate) async fn connect(bucket: String) -> anyhow::Result<Self> {
        let endpoint = env_or("S3_ENDPOINT", DEFAULT_ENDPOINT);
        let access_key = env_or("S3_ACCESS_KEY", DEFAULT_ACCESS_KEY);
        let secret_key = env_or("S3_SECRET_KEY", DEFAULT_SECRET_KEY);
        let config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(REGION))
            .endpoint_url(&endpoint)
            .credentials_provider(Credentials::new(
                &access_key,
                &secret_key,
                None,
                None,
                "xtask-e2e",
            ))
            .force_path_style(true)
            .build();
        let client = Client::from_conf(config);

        client.list_buckets().send().await.with_context(|| {
            format!("S3 at {endpoint} is unreachable (hint: `cargo xtask setup minio`)")
        })?;
        client
            .create_bucket()
            .bucket(&bucket)
            .send()
            .await
            .with_context(|| format!("failed to create bucket {bucket}"))?;
        println!("e2e s3: {endpoint} (bucket {bucket})");

        Ok(Self {
            client,
            endpoint,
            access_key,
            secret_key,
            bucket,
        })
    }

    /// The environment that points an AWS SDK client (the archiver) at the
    /// same store and bucket.
    pub(crate) fn service_env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("S3_BUCKET", self.bucket.clone()),
            ("S3_ENDPOINT", self.endpoint.clone()),
            ("S3_FORCE_PATH_STYLE", "true".to_owned()),
            ("AWS_ACCESS_KEY_ID", self.access_key.clone()),
            ("AWS_SECRET_ACCESS_KEY", self.secret_key.clone()),
            ("AWS_REGION", REGION.to_owned()),
        ]
    }

    /// Every key under `prefix`, sorted.
    pub(crate) async fn list(&self, prefix: &str) -> anyhow::Result<Vec<String>> {
        let pages = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .into_paginator()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await
            .with_context(|| format!("failed to list s3://{}/{prefix}", self.bucket))?;
        let mut keys: Vec<String> = pages
            .iter()
            .flat_map(|page| page.contents())
            .filter_map(|object| object.key())
            .map(str::to_owned)
            .collect();
        keys.sort_unstable();
        Ok(keys)
    }

    /// Polls until at least `count` objects exist under `prefix` and returns
    /// their keys; fails, naming what it found, once `timeout` elapses.
    pub(crate) async fn wait_for(
        &self,
        prefix: &str,
        count: usize,
        timeout: Duration,
    ) -> anyhow::Result<Vec<String>> {
        let deadline = Instant::now() + timeout;
        loop {
            let keys = self.list(prefix).await?;
            if keys.len() >= count {
                return Ok(keys);
            }
            if Instant::now() >= deadline {
                bail!(
                    "expected {count} object(s) under {prefix} within {timeout:?}, found {keys:?}"
                );
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Fetches `key`, zstd-decompressing its body.
    pub(crate) async fn get(&self, key: &str) -> anyhow::Result<Object> {
        let object = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .with_context(|| format!("failed to fetch {key}"))?;
        let metadata = object.metadata().cloned().unwrap_or_default();
        let compressed = object
            .body
            .collect()
            .await
            .with_context(|| format!("failed to download {key}"))?
            .into_bytes();
        let body = zstd::decode_all(&compressed[..])
            .with_context(|| format!("{key} is not zstd-compressed"))?;
        Ok(Object { body, metadata })
    }

    /// Writes `body` to `key` with `If-None-Match: *`. Returns `false` when
    /// the store answers 412 (the key already exists).
    pub(crate) async fn put_if_absent(&self, key: &str, body: Vec<u8>) -> anyhow::Result<bool> {
        let put = self
            .client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .if_none_match("*")
            .body(ByteStream::from(body))
            .send()
            .await;
        match put {
            Ok(_) => Ok(true),
            // A failed conditional write has no modeled error, so classify
            // it by the raw status.
            Err(err)
                if err
                    .raw_response()
                    .is_some_and(|response| response.status().as_u16() == 412) =>
            {
                Ok(false)
            }
            Err(err) => Err(err).with_context(|| format!("failed to put {key}")),
        }
    }

    /// Empties and deletes the per-run bucket.
    pub(crate) async fn delete_bucket(&self) -> anyhow::Result<()> {
        let keys = self.list("").await?;
        for chunk in keys.chunks(1000) {
            let objects = chunk
                .iter()
                .map(|key| ObjectIdentifier::builder().key(key).build())
                .collect::<Result<Vec<_>, _>>()?;
            self.client
                .delete_objects()
                .bucket(&self.bucket)
                .delete(Delete::builder().set_objects(Some(objects)).build()?)
                .send()
                .await
                .context("failed to empty the e2e bucket")?;
        }
        self.client
            .delete_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .context("failed to delete the e2e bucket")?;
        Ok(())
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}
