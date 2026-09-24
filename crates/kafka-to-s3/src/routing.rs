//! Lenient routing parse (D17) and quarantine classification (D12).

use chrono::{DateTime, Timelike, Utc};
use serde::Deserialize;

/// S3 segregation key: one file per `(tenant, workspace, dt, hour)` per flush (D3).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct GroupKey {
    pub tenant: String,
    pub workspace: String,
    /// UTC bucket date, `YYYY-MM-DD`.
    pub dt: String,
    /// UTC bucket hour, 0-23.
    pub hour: u32,
}

/// Where a consumed payload routes: a group key, or quarantine.
#[derive(Debug, PartialEq)]
pub enum Classification {
    Routed(GroupKey),
    Quarantined(QuarantineReason),
}

/// Why a payload cannot be routed (D12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReason {
    /// Payload is not valid JSON.
    InvalidJson,
    /// `tenant_id` is missing or empty.
    MissingTenantId,
    /// Neither `arrived_at` nor `occured_at` is present and parseable (D6).
    MissingTimestamp,
}

/// Classify a raw Kafka payload into its archive destination.
///
/// Quarantined payloads are logged (`error!`) here; they still count toward the batch
/// cycle and have their offsets committed past (D12 poison-pill protection).
pub fn classify(payload: &[u8]) -> Classification {
    let raw: RawRoutingMeta = match serde_json::from_slice(payload) {
        Ok(raw) => raw,
        Err(error) => {
            tracing::error!(
                error = %error,
                bytes = payload.len(),
                preview = %preview(payload),
                "quarantining payload: not valid JSON"
            );
            return Classification::Quarantined(QuarantineReason::InvalidJson);
        }
    };
    let Some(tenant) = non_empty(raw.tenant_id) else {
        tracing::error!(
            bytes = payload.len(),
            preview = %preview(payload),
            "quarantining payload: tenant_id missing"
        );
        return Classification::Quarantined(QuarantineReason::MissingTenantId);
    };
    let Some(bucket) = hour_bucket(raw.arrived_at.as_deref(), raw.occured_at.as_deref()) else {
        tracing::error!(
            tenant,
            bytes = payload.len(),
            "quarantining payload: no parseable arrived_at or occured_at"
        );
        return Classification::Quarantined(QuarantineReason::MissingTimestamp);
    };
    let workspace = match non_empty(raw.workspace_id) {
        Some(workspace) => workspace,
        None => {
            tracing::warn!(tenant, "workspace_id missing; routing to 'default' (D11)");
            "default".to_string()
        }
    };
    Classification::Routed(GroupKey {
        tenant,
        workspace,
        dt: bucket.format("%Y-%m-%d").to_string(),
        hour: bucket.hour(),
    })
}

/// Lenient parse projection of a canonical event: only routing fields, unknown
/// fields ignored (D17). Timestamps stay strings here so one bad field degrades
/// gracefully instead of failing the whole parse.
#[derive(Deserialize)]
struct RawRoutingMeta {
    arrived_at: Option<String>,
    occured_at: Option<String>,
    tenant_id: Option<String>,
    workspace_id: Option<String>,
}

/// Resolve the hour-bucket timestamp: `arrived_at` UTC, else `occured_at` with a
/// warning; `None` when neither is usable (D6).
fn hour_bucket(arrived_at: Option<&str>, occured_at: Option<&str>) -> Option<DateTime<Utc>> {
    if let Some(arrived) = parse_timestamp(arrived_at) {
        return Some(arrived);
    }
    let fallback = parse_timestamp(occured_at);
    if fallback.is_some() {
        tracing::warn!("arrived_at absent or unparseable; bucketing by occured_at");
    }
    fallback
}

fn parse_timestamp(value: Option<&str>) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value?)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.is_empty())
}

/// Capped lossy preview for quarantine diagnostics.
fn preview(payload: &[u8]) -> String {
    let cap = payload.len().min(64);
    String::from_utf8_lossy(&payload[..cap]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify_str(payload: &str) -> Classification {
        classify(payload.as_bytes())
    }

    #[test]
    fn valid_event_routes_by_arrived_at() {
        let classification = classify_str(
            r#"{"id":"e1","tenant_id":"merchant-1","workspace_id":"ws-9",
                "occured_at":"garbage-timestamp","arrived_at":"2026-09-24T03:35:00Z"}"#,
        );
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(key.tenant, "merchant-1");
        assert_eq!(key.workspace, "ws-9");
        assert_eq!(key.dt, "2026-09-24");
        assert_eq!(key.hour, 3);
    }

    #[test]
    fn arrived_at_offset_is_converted_to_utc() {
        let classification = classify_str(
            r#"{"tenant_id":"t","occured_at":"2026-09-24T10:00:00Z","arrived_at":"2026-09-24T09:05:00+05:30"}"#,
        );
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(key.dt, "2026-09-24");
        assert_eq!(key.hour, 3); // 09:05+05:30 == 03:35Z
    }

    #[test]
    fn absent_arrived_at_falls_back_to_occured_at() {
        let classification =
            classify_str(r#"{"tenant_id":"t","occured_at":"2026-01-02T23:59:59Z"}"#);
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(key.dt, "2026-01-02");
        assert_eq!(key.hour, 23);
    }

    #[test]
    fn unparseable_arrived_at_falls_back_to_occured_at() {
        let classification = classify_str(
            r#"{"tenant_id":"t","occured_at":"2026-01-02T23:59:59Z","arrived_at":"nope"}"#,
        );
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(key.hour, 23);
    }

    #[test]
    fn missing_workspace_defaults() {
        let classification =
            classify_str(r#"{"tenant_id":"t","occured_at":"2026-01-02T23:59:59Z"}"#);
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(key.workspace, "default");
    }

    #[test]
    fn missing_tenant_id_quarantines() {
        let classification = classify_str(r#"{"occured_at":"2026-01-02T23:59:59Z"}"#);
        assert_eq!(
            classification,
            Classification::Quarantined(QuarantineReason::MissingTenantId)
        );
    }

    #[test]
    fn empty_tenant_id_quarantines() {
        let classification =
            classify_str(r#"{"tenant_id":"","occured_at":"2026-01-02T23:59:59Z"}"#);
        assert_eq!(
            classification,
            Classification::Quarantined(QuarantineReason::MissingTenantId)
        );
    }

    #[test]
    fn non_json_payload_quarantines() {
        assert_eq!(
            classify_str("not json at all"),
            Classification::Quarantined(QuarantineReason::InvalidJson)
        );
        assert_eq!(
            classify(b"\xff\xfe\x00binary"),
            Classification::Quarantined(QuarantineReason::InvalidJson)
        );
        assert_eq!(
            classify(b""),
            Classification::Quarantined(QuarantineReason::InvalidJson)
        );
    }

    #[test]
    fn missing_or_unparseable_timestamps_quarantine() {
        for payload in [
            r#"{"tenant_id":"t"}"#,
            r#"{"tenant_id":"t","occured_at":"garbage"}"#,
            r#"{"tenant_id":"t","arrived_at":"garbage","occured_at":"also-garbage"}"#,
        ] {
            assert_eq!(
                classify_str(payload),
                Classification::Quarantined(QuarantineReason::MissingTimestamp),
                "{payload}"
            );
        }
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let classification = classify_str(
            r#"{"envelop_version":"1.0","id":"x","name":"n","anon_id":"a",
                "future_field":{"nested":[1,2,3]},"tenant_id":"t","workspace_id":"w",
                "occured_at":"2026-03-04T05:06:07Z","arrived_at":"2026-03-04T05:06:08Z"}"#,
        );
        let Classification::Routed(key) = classification else {
            panic!("expected routed, got {classification:?}");
        };
        assert_eq!(
            key,
            GroupKey {
                tenant: "t".to_string(),
                workspace: "w".to_string(),
                dt: "2026-03-04".to_string(),
                hour: 5,
            }
        );
    }
}
