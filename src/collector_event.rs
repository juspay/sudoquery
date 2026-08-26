use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Geo {
    pub country: Option<String>,
}

#[derive(Clone, Copy, Default, Deserialize, Serialize)]
pub enum EnvelopVersion {
    #[default]
    #[serde(rename = "1.0")]
    V1,
}

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
    pub tenant_id: String,
    pub workspace_id: Option<String>,
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
