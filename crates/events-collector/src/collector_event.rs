pub use canonical_event::{EnvelopVersion, Geo};
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct SystemProperties {
    pub geo: Option<Geo>,
    pub timezone: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Clone, Deserialize)]
pub struct CollectorEvent {
    pub envelop_version: EnvelopVersion,
    pub id: uuid::Uuid,
    pub name: String,
    pub org_id: String,
    pub proj_id: Option<String>,
    pub session_id: Option<String>,
    pub anon_id: String,
    pub actor_id: Option<String>,
    pub source: Option<String>,
    pub occured_at: chrono::DateTime<chrono::Utc>,
    pub properties: Option<serde_json::Value>,

    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub system_properties: Option<SystemProperties>,
}

#[derive(Clone, Deserialize)]
pub struct Batch {
    pub events: Vec<CollectorEvent>,
    pub system_properties: Option<SystemProperties>,
}
