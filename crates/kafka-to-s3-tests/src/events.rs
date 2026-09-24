//! Event factory for the harness.

use canonical_event::CanonicalEvent;
use chrono::{DateTime, Utc};

/// Name given to every harness-produced event.
pub const EVENT_NAME: &str = "archiver_test_event";

/// Generates a tenant id unique to this call.
///
/// The topic and bucket are shared across test runs, so every test derives
/// its own S3 prefix namespace from a unique tenant; Kafka offsets are
/// correspondingly not assumed to start at 0.
pub fn unique_tenant(prefix: &str) -> String {
    // `simple()` always renders 32 ASCII hex characters, so slicing at 8 is
    // safe.
    let suffix = uuid::Uuid::new_v4().simple().to_string();
    format!("{prefix}-{}", &suffix[..8])
}

/// Builds a canonical event for `tenant`/`workspace` arriving at `arrived_at`.
///
/// `seq` is an opaque marker stored in the event's `properties` and used to
/// make the `anon_id` unique. The event always carries the intended tenant,
/// workspace, and arrival time; the builder fills `envelop_version`,
/// `occured_at`, and the remaining optional fields with defaults.
pub fn make_event(
    tenant: &str,
    workspace: &str,
    arrived_at: DateTime<Utc>,
    seq: u64,
) -> CanonicalEvent {
    CanonicalEvent::builder()
        .id(uuid::Uuid::new_v4())
        .name(EVENT_NAME.to_string())
        .tenant_id(tenant.to_string())
        .workspace_id(Some(workspace.to_string()))
        .anon_id(format!("anon-{seq}"))
        .arrived_at(Some(arrived_at))
        .properties(Some(serde_json::json!({ "seq": seq })))
        .build()
}

/// Returns the `id` of a built canonical event.
///
/// `CanonicalEvent` keeps `id` private with no accessor, so the harness reads
/// it back from the event's JSON serialization (the same wire format the
/// service consumes).
pub fn event_id(event: &CanonicalEvent) -> uuid::Uuid {
    let value = serde_json::to_value(event).expect("failed to serialize canonical event");
    let raw = value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("canonical event has no string 'id': {value}"));
    uuid::Uuid::parse_str(raw)
        .unwrap_or_else(|err| panic!("canonical event 'id' {raw:?} is not a valid UUID: {err}"))
}
