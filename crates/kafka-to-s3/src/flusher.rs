//! S3 uploads: conditional PUT with retry and backoff (D9, D10, D13), plus the
//! engine sink that uploads then commits (D8, D14, D15).

use crate::buffer::{Batch, Entry};
use crate::config::Config;
use crate::consumer::EventConsumer;
use crate::engine::Sink;
use crate::object;
use crate::routing::GroupKey;
use aws_sdk_s3::Client;
use aws_sdk_s3::error::{ProvideErrorMetadata, SdkError};
use aws_sdk_s3::operation::put_object::PutObjectError;
use aws_sdk_s3::primitives::ByteStream;
use aws_smithy_runtime_api::http::Response;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Total upload attempts before giving up (D13).
const MAX_UPLOAD_ATTEMPTS: u32 = 5;
/// Base delay for the exponential backoff (D13: 1s).
const BACKOFF_BASE_SECS: u32 = 1;

/// Upload failure after retries are exhausted (D13) or compression failure.
#[derive(Debug, thiserror::Error)]
pub enum FlushError {
    #[error("failed to compress batch: {0}")]
    Compress(#[from] std::io::Error),
    #[error("S3 upload of {key} failed after {attempts} attempts: {source}")]
    UploadExhausted {
        key: String,
        attempts: u32,
        source: Box<SdkError<PutObjectError>>,
    },
}

pub struct Flusher {
    s3: Client,
    bucket: String,
    prefix: Option<String>,
    source_id: Option<String>,
}

impl Flusher {
    /// Build from boot config; honors `S3_ENDPOINT` + `S3_FORCE_PATH_STYLE` (MinIO).
    pub async fn new(config: &Config) -> Self {
        let sdk_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
        let mut builder = aws_sdk_s3::config::Builder::from(&sdk_config);
        if let Some(endpoint) = &config.s3_endpoint {
            builder = builder
                .endpoint_url(endpoint)
                .force_path_style(config.s3_force_path_style);
        }
        Self {
            s3: Client::from_conf(builder.build()),
            bucket: config.s3_bucket.clone(),
            prefix: config.s3_prefix.clone(),
            source_id: config.source_id.clone(),
        }
    }

    /// Upload one flush cycle: every group and every quarantine entry (D3, D12).
    ///
    /// # Errors
    /// Propagates [`FlushError`] when compression fails or any upload exhausts
    /// its retries (D13: the caller exits 1, offsets stay uncommitted, replay follows).
    pub async fn flush(
        &self,
        groups: Vec<(GroupKey, Vec<Entry>)>,
        quarantine: Vec<Entry>,
    ) -> Result<(), FlushError> {
        let created_at = Utc::now();
        for (group, entries) in &groups {
            self.put_group(group, entries, created_at).await?;
        }
        for entry in &quarantine {
            self.put_quarantine(entry, created_at).await?;
        }
        Ok(())
    }

    async fn put_group(
        &self,
        group: &GroupKey,
        entries: &[Entry],
        created_at: DateTime<Utc>,
    ) -> Result<(), FlushError> {
        let body = object::jsonl_body(entries);
        let hash = object::content_hash16(&body);
        let key = object::group_object_key(self.prefix.as_deref(), group, &hash);
        let metadata = object::object_metadata(self.source_id.as_deref(), entries, created_at);
        let compressed = object::compress(&body)?;
        self.put_with_retry(&key, &compressed, &metadata).await?;
        tracing::info!(key = %key, events = entries.len(), "archived group object");
        Ok(())
    }

    /// One quarantined event per object, keyed by the hash of its own bytes (D12):
    /// identical poison pills collide to the same key and dedupe naturally.
    async fn put_quarantine(
        &self,
        entry: &Entry,
        created_at: DateTime<Utc>,
    ) -> Result<(), FlushError> {
        let entries = std::slice::from_ref(entry);
        let body = object::jsonl_body(entries);
        let hash = object::content_hash16(&body);
        let key = object::quarantine_object_key(self.prefix.as_deref(), &hash);
        let metadata = object::object_metadata(self.source_id.as_deref(), entries, created_at);
        let compressed = object::compress(&body)?;
        self.put_with_retry(&key, &compressed, &metadata).await?;
        Ok(())
    }

    /// Conditional `PutObject` with 5-attempt exponential backoff (D9, D13). A
    /// `412 PreconditionFailed` means the object already exists with the same
    /// content hash, so it is logged and treated as success.
    async fn put_with_retry(
        &self,
        key: &str,
        body: &[u8],
        metadata: &HashMap<String, String>,
    ) -> Result<(), FlushError> {
        let mut attempt: u32 = 1;
        loop {
            let error = match self.put_once(key, body, metadata).await {
                Ok(()) => return Ok(()),
                Err(error) => error,
            };
            if is_precondition_failed(error.as_ref()) {
                tracing::info!(
                    key,
                    "object already exists; conditional PUT treated as success"
                );
                return Ok(());
            }
            if attempt >= MAX_UPLOAD_ATTEMPTS {
                return Err(FlushError::UploadExhausted {
                    key: key.to_string(),
                    attempts: MAX_UPLOAD_ATTEMPTS,
                    source: error,
                });
            }
            let delay = backoff_delay(attempt, jitter_fraction());
            tracing::warn!(
                key,
                attempt,
                delay_secs = delay.as_secs_f64(),
                error = %error,
                "S3 upload failed; retrying with backoff"
            );
            tokio::time::sleep(delay).await;
            attempt += 1;
        }
    }

    async fn put_once(
        &self,
        key: &str,
        body: &[u8],
        metadata: &HashMap<String, String>,
    ) -> Result<(), Box<SdkError<PutObjectError>>> {
        let mut request = self
            .s3
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(body.to_vec()))
            .if_none_match("*");
        for (name, value) in metadata {
            request = request.metadata(name.as_str(), value.as_str());
        }
        request.send().await.map(|_| ()).map_err(Box::new)
    }
}

/// Engine sink: uploads via [`Flusher`], then commits offsets (D8: only after all
/// uploads in the cycle succeeded).
pub struct ArchiveSink<'a> {
    flusher: Flusher,
    consumer: &'a EventConsumer,
}

impl<'a> ArchiveSink<'a> {
    pub fn new(flusher: Flusher, consumer: &'a EventConsumer) -> Self {
        Self { flusher, consumer }
    }
}

impl Sink for ArchiveSink<'_> {
    async fn flush(&mut self, batch: Batch) -> Result<(), FlushError> {
        self.flusher.flush(batch.groups, batch.quarantine).await?;
        // Best-effort commit (D14): logged, offsets replayed on failure.
        if let Err(error) = self.consumer.commit_async(&batch.commits) {
            tracing::warn!(%error, "offset commit failed (best-effort; events may be reprocessed)");
        }
        Ok(())
    }

    async fn flush_final(&mut self, batch: Batch) -> Result<(), FlushError> {
        self.flusher.flush(batch.groups, batch.quarantine).await?;
        if let Err(error) = self.consumer.commit_sync(&batch.commits) {
            tracing::warn!(%error, "final offset commit failed (best-effort; events may be reprocessed)");
        }
        Ok(())
    }
}

/// A `412` means the same-hash object is already there — same content, treat as success (D9).
///
/// Detection is dual-path: MinIO answers failed conditional writes either
/// with an XML body (surfacing a modeled/generic `PreconditionFailed` error
/// code) or with a bare `412` status and no parseable body (no code in the
/// error metadata at all). The metadata-code check alone misses the bare
/// variant, so the raw HTTP status is checked as well.
fn is_precondition_failed<E: ProvideErrorMetadata>(error: &SdkError<E, Response>) -> bool {
    let modeled_code = error
        .as_service_error()
        .and_then(|service_error| service_error.meta().code());
    modeled_code == Some("PreconditionFailed")
        || error
            .raw_response()
            .is_some_and(|response| response.status().as_u16() == 412)
}

/// Exponential backoff with jitter (D13): `1s * 2^(attempt-1)`, scaled to
/// `[50%, 100%]` of nominal by `jitter_fraction` in `[0, 0.5)`.
fn backoff_delay(attempt: u32, jitter_fraction: f64) -> Duration {
    let exponent = attempt.saturating_sub(1).min(16);
    let nominal_secs = f64::from(BACKOFF_BASE_SECS << exponent);
    Duration::from_secs_f64(nominal_secs * (1.0 - jitter_fraction.min(0.5)))
}

/// Jitter fraction in `[0, 0.5)` derived from wall-clock sub-second nanoseconds.
fn jitter_fraction() -> f64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since_epoch| since_epoch.subsec_nanos())
        .unwrap_or(0);
    f64::from(nanos) / 2e9
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::error::ErrorMetadata;
    use aws_sdk_s3::primitives::SdkBody;
    use aws_smithy_runtime_api::http::{Response, StatusCode};

    fn precondition_error(code: Option<&str>) -> SdkError<PutObjectError> {
        let mut metadata = ErrorMetadata::builder();
        if let Some(code) = code {
            metadata = metadata.code(code);
        }
        let service_error = PutObjectError::generic(metadata.build());
        SdkError::service_error(service_error, raw_response(400))
    }

    fn raw_response(status: u16) -> Response {
        Response::new(
            StatusCode::try_from(status).expect("status within 100..1000"),
            SdkBody::empty(),
        )
    }

    #[test]
    fn detects_precondition_failed_service_error() {
        assert!(is_precondition_failed(&precondition_error(Some(
            "PreconditionFailed"
        ))));
    }

    #[test]
    fn other_service_errors_are_not_precondition_failed() {
        assert!(!is_precondition_failed(&precondition_error(Some(
            "InternalError"
        ))));
        assert!(!is_precondition_failed(&precondition_error(None)));
    }

    #[test]
    fn detects_bare_412_status_without_parseable_body() {
        let error: SdkError<PutObjectError> =
            SdkError::response_error("no body", raw_response(412));
        assert!(is_precondition_failed(&error));
    }

    #[test]
    fn detects_unhandled_412_without_modeled_code() {
        let error = SdkError::service_error(
            PutObjectError::unhandled("minio xml error body"),
            raw_response(412),
        );
        assert!(is_precondition_failed(&error));
    }

    #[test]
    fn other_statuses_are_not_precondition_failed() {
        let response_error: SdkError<PutObjectError> =
            SdkError::response_error("no body", raw_response(500));
        assert!(!is_precondition_failed(&response_error));
        let unhandled = SdkError::service_error(
            PutObjectError::unhandled("minio xml error body"),
            raw_response(409),
        );
        assert!(!is_precondition_failed(&unhandled));
    }

    #[test]
    fn backoff_delay_doubles_per_attempt_without_jitter() {
        assert_eq!(backoff_delay(1, 0.0), Duration::from_secs(1));
        assert_eq!(backoff_delay(2, 0.0), Duration::from_secs(2));
        assert_eq!(backoff_delay(3, 0.0), Duration::from_secs(4));
        assert_eq!(backoff_delay(4, 0.0), Duration::from_secs(8));
    }

    #[test]
    fn backoff_delay_stays_between_half_and_full_nominal() {
        assert_eq!(backoff_delay(1, 0.5), Duration::from_millis(500));
        let half = backoff_delay(3, 0.5);
        let full = backoff_delay(3, 0.0);
        assert!(half >= Duration::from_secs(2) && half < full, "{half:?}");
    }

    #[test]
    fn jitter_fraction_stays_in_expected_band() {
        for _ in 0..100 {
            let fraction = jitter_fraction();
            assert!((0.0..0.5).contains(&fraction), "{fraction}");
        }
    }
}
