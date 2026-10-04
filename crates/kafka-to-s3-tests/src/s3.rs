//! MinIO/S3 access to the service's archive bucket.

use std::time::{Duration, Instant};

use aws_sdk_s3::Client;
use aws_sdk_s3::config::{BehaviorVersion, Credentials, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::types::BucketLocationConstraint;
use aws_sdk_s3::types::CreateBucketConfiguration;
use serde::Deserialize;

use crate::config::TestConfig;

/// How often [`ArchiveStore::wait_for_objects`] polls MinIO.
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// An archived canonical event parsed from a JSONL object.
///
/// `CanonicalEvent` itself does not implement `Deserialize`, so the harness
/// parses the fields its assertions need into this local type at the boundary.
#[derive(Debug, Deserialize)]
pub struct ArchivedEvent {
    /// The event's canonical `id`.
    pub id: uuid::Uuid,
    /// Org the event was segregated under.
    pub org_id: String,
    /// Project the event was segregated under.
    pub proj_id: Option<String>,
    /// Arrival time the event was segregated under (UTC).
    pub arrived_at: chrono::DateTime<chrono::Utc>,
}

/// MinIO/S3 client for the bucket the service archives into.
pub struct ArchiveStore {
    client: Client,
    bucket: String,
    region: String,
}

impl ArchiveStore {
    /// Builds a client for the endpoint, credentials, region, and bucket
    /// named by the `K2S_TEST_S3_*` environment variables.
    pub fn new(config: &TestConfig) -> ArchiveStore {
        let s3_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new(config.s3_region.clone()))
            .endpoint_url(config.s3_endpoint.clone())
            .credentials_provider(Credentials::new(
                config.s3_access_key.clone(),
                config.s3_secret_key.clone(),
                None,
                None,
                "static",
            ))
            .force_path_style(true)
            .build();
        ArchiveStore {
            client: Client::from_conf(s3_config),
            bucket: config.s3_bucket.clone(),
            region: config.s3_region.clone(),
        }
    }

    /// Creates the archive bucket if it does not exist yet.
    ///
    /// For `us-east-1` no `CreateBucketConfiguration` is sent, because MinIO
    /// rejects the us-east-1 location constraint. A concurrent run creating
    /// the same bucket is tolerated.
    pub async fn ensure_bucket(&self) {
        if self
            .client
            .head_bucket()
            .bucket(&self.bucket)
            .send()
            .await
            .is_ok()
        {
            return;
        }

        let mut request = self.client.create_bucket().bucket(&self.bucket);
        if !self.region.eq_ignore_ascii_case("us-east-1") {
            request = request.create_bucket_configuration(
                CreateBucketConfiguration::builder()
                    .location_constraint(BucketLocationConstraint::from(self.region.as_str()))
                    .build(),
            );
        }
        match request.send().await {
            Ok(_) => {}
            Err(err)
                if err.as_service_error().is_some_and(|service_error| {
                    service_error.is_bucket_already_owned_by_you()
                        || service_error.is_bucket_already_exists()
                }) =>
            {
                // Another run created the bucket between the head and the
                // create; the bucket exists, which is all we need.
            }
            Err(err) => panic!(
                "failed to create bucket {} (K2S_TEST_S3_BUCKET) at endpoint: {err}",
                self.bucket
            ),
        }
    }

    /// Lists all object keys under `prefix`, sorted lexicographically.
    pub async fn list_keys(&self, prefix: &str) -> Vec<String> {
        let pages = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(prefix)
            .into_paginator()
            .send()
            .collect::<Result<Vec<_>, _>>()
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "failed to list objects under s3://{}/{}: {err}",
                    self.bucket, prefix
                )
            });
        let mut keys: Vec<String> = pages
            .iter()
            .flat_map(|page| page.contents())
            .filter_map(|object| object.key())
            .map(str::to_string)
            .collect();
        keys.sort();
        keys
    }

    /// Polls every 2s until at least `expected_count` objects exist under
    /// `prefix`, then returns the keys found.
    ///
    /// Panics when `deadline` elapses first, listing the keys found so far to
    /// make the failure diagnosable.
    pub async fn wait_for_objects(
        &self,
        prefix: &str,
        expected_count: usize,
        deadline: Duration,
    ) -> Vec<String> {
        let started = Instant::now();
        loop {
            let keys = self.list_keys(prefix).await;
            if keys.len() >= expected_count {
                return keys;
            }
            if started.elapsed() >= deadline {
                panic!(
                    "timed out after {deadline:?} waiting for {expected_count} object(s) under \
                     s3://{}/{prefix}; found {}: {keys:?}",
                    self.bucket,
                    keys.len()
                );
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    }

    /// Fetches `key`, decompresses its zstd payload, and parses the JSONL
    /// lines into [`ArchivedEvent`]s.
    pub async fn fetch_jsonl(&self, key: &str) -> Vec<ArchivedEvent> {
        let object = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .unwrap_or_else(|err| panic!("failed to fetch s3://{}/{}: {err}", self.bucket, key));
        let bytes = object
            .body
            .collect()
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "failed to download body of s3://{}/{}: {err}",
                    self.bucket, key
                )
            })
            .into_bytes();
        let decompressed = zstd::decode_all(&bytes[..]).unwrap_or_else(|err| {
            panic!(
                "failed to zstd-decompress s3://{}/{}: {err}",
                self.bucket, key
            )
        });
        let text = std::str::from_utf8(&decompressed).unwrap_or_else(|err| {
            panic!(
                "s3://{}/{} is not valid UTF-8 JSONL: {err}",
                self.bucket, key
            )
        });
        text.lines()
            .map(|line| {
                serde_json::from_str::<ArchivedEvent>(line).unwrap_or_else(|err| {
                    panic!(
                        "failed to parse archived event from s3://{}/{} line {line:?}: {err}",
                        self.bucket, key
                    )
                })
            })
            .collect()
    }

    /// Fetches `key`'s S3 object metadata via a HEAD request.
    pub async fn head_object(&self, key: &str) -> ObjectMetadata {
        let output = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .unwrap_or_else(|err| panic!("failed to HEAD s3://{}/{}: {err}", self.bucket, key));
        let metadata = output.metadata();
        let field = |name: &str| {
            metadata
                .and_then(|map| map.get(name))
                .map(|value| value.to_string())
        };
        ObjectMetadata {
            source: field("source"),
            event_count: field("event-count"),
            ranges: field("ranges"),
        }
    }

    /// Uploads `body` to `key` with a conditional `If-None-Match: *` write.
    ///
    /// Returns `false` when the store answers `412 PreconditionFailed` —
    /// the outcome the service treats as success, since an identical
    /// content-hash key implies identical content. Any other error panics.
    pub async fn put_if_absent(&self, key: &str, body: Vec<u8>) -> bool {
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
            Ok(_) => true,
            // A 412 is the outcome the service treats as success: an
            // identical content-hash key implies identical content. S3
            // answers a failed conditional write with a bare 412 status and
            // no modeled error body, MinIO with an XML body; both keep the
            // raw response, so the status is the reliable classifier.
            Err(err)
                if err
                    .raw_response()
                    .is_some_and(|response| response.status().as_u16() == 412) =>
            {
                false
            }
            Err(err) => panic!("failed to put s3://{}/{}: {err}", self.bucket, key),
        }
    }
}

/// Provenance metadata the service attaches to every archived object,
/// read back with [`ArchiveStore::head_object`].
#[derive(Debug, Default)]
pub struct ObjectMetadata {
    /// `source` — the service's `SOURCE_ID`, present only when configured.
    pub source: Option<String>,
    /// `event-count` — the number of JSONL lines in the object.
    pub event_count: Option<String>,
    /// `ranges` — `topic:partition:first:last` entries joined by `;`.
    pub ranges: Option<String>,
}

/// Returns `true` when `file_name` matches the archived-object file-name
/// form: exactly 16 lowercase hex characters (the first 16 hex characters of
/// the sha256 of the file's uncompressed JSONL content) followed by
/// `.jsonl.zst`.
pub fn is_hash_file_name(file_name: &str) -> bool {
    let Some(stem) = file_name.strip_suffix(".jsonl.zst") else {
        return false;
    };
    stem.len() == 16
        && stem
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::is_hash_file_name;

    #[test]
    fn accepts_sixteen_lowercase_hex_chars() {
        assert!(is_hash_file_name("0123456789abcdef.jsonl.zst"));
        assert!(is_hash_file_name("ffffffffffffffff.jsonl.zst"));
    }

    #[test]
    fn rejects_uppercase_hex_chars() {
        assert!(!is_hash_file_name("0123456789ABCDEF.jsonl.zst"));
    }

    #[test]
    fn rejects_wrong_hash_length() {
        assert!(!is_hash_file_name("0123456789abcde.jsonl.zst"));
        assert!(!is_hash_file_name("0123456789abcdeff.jsonl.zst"));
    }

    #[test]
    fn rejects_non_hex_chars() {
        assert!(!is_hash_file_name("0123456789abcdeg.jsonl.zst"));
        assert!(!is_hash_file_name("first-last.jsonl.zst"));
    }

    #[test]
    fn rejects_missing_jsonl_zst_suffix() {
        assert!(!is_hash_file_name("0123456789abcdef.jsonl"));
        assert!(!is_hash_file_name("0123456789abcdef"));
    }
}
