use chrono::SecondsFormat;
use serde::{Deserialize, Serialize, Serializer};
use serde_with::skip_serializing_none;

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

#[skip_serializing_none]
#[derive(Deserialize, Serialize)]
pub struct CanonicalEvent {
    envelop_version: EnvelopVersion,
    id: uuid::Uuid,
    name: String,
    occured_at: chrono::DateTime<chrono::Utc>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        serialize_with = "serialize_optional_utc_datetime_nanos"
    )]
    pub arrived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub org_id: String,
    pub proj_id: Option<String>,
    pub session_id: Option<String>,
    pub anon_id: String,
    pub actor_id: Option<String>,
    pub source: Option<String>,
    pub correlation_id: Option<String>,
    pub trace_id: Option<String>,
    pub authenticated: Option<bool>,
    pub properties: Option<serde_json::Value>,
    pub system_properties: Option<SystemProperties>,
}

impl CanonicalEvent {
    pub fn builder() -> CanonicalEventBuilder {
        CanonicalEventBuilder::default()
    }

    pub fn id(&self) -> uuid::Uuid {
        self.id
    }
}

fn serialize_optional_utc_datetime_nanos<S>(
    value: &Option<chrono::DateTime<chrono::Utc>>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match value {
        Some(value) => serializer.serialize_str(&value.to_rfc3339_opts(SecondsFormat::Nanos, true)),
        None => serializer.serialize_none(),
    }
}

#[skip_serializing_none]
#[derive(Default, Deserialize, Serialize)]
pub struct SystemProperties {
    pub geo: Option<Geo>,
    pub timezone: Option<String>,
    pub ip_address: Option<String>,
}

impl SystemProperties {
    pub fn builder() -> SystemPropertiesBuilder {
        SystemPropertiesBuilder::default()
    }
}

#[derive(Default)]
pub struct SystemPropertiesBuilder {
    geo: Option<Geo>,
    timezone: Option<String>,
    ip_address: Option<String>,
}

impl SystemPropertiesBuilder {
    pub fn geo(mut self, geo: Option<Geo>) -> Self {
        self.geo = geo;
        self
    }

    pub fn timezone(mut self, timezone: Option<String>) -> Self {
        self.timezone = timezone;
        self
    }

    pub fn ip_address(mut self, ip_address: Option<String>) -> Self {
        self.ip_address = ip_address;
        self
    }

    pub fn build(self) -> SystemProperties {
        SystemProperties {
            geo: self.geo,
            timezone: self.timezone,
            ip_address: self.ip_address,
        }
    }
}

#[derive(Default)]
pub struct CanonicalEventBuilder {
    envelop_version: EnvelopVersion,
    id: Option<uuid::Uuid>,
    name: String,
    occured_at: Option<chrono::DateTime<chrono::Utc>>,
    arrived_at: Option<chrono::DateTime<chrono::Utc>>,
    org_id: String,
    proj_id: Option<String>,
    session_id: Option<String>,
    anon_id: String,
    actor_id: Option<String>,
    source: Option<String>,
    correlation_id: Option<String>,
    trace_id: Option<String>,
    authenticated: Option<bool>,
    properties: Option<serde_json::Value>,
    system_properties: Option<SystemProperties>,
}

impl CanonicalEventBuilder {
    pub fn envelop_version(mut self, envelop_version: EnvelopVersion) -> Self {
        self.envelop_version = envelop_version;
        self
    }

    pub fn id(mut self, id: uuid::Uuid) -> Self {
        self.id = Some(id);
        self
    }

    pub fn name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn occured_at(mut self, occured_at: chrono::DateTime<chrono::Utc>) -> Self {
        self.occured_at = Some(occured_at);
        self
    }

    pub fn arrived_at(mut self, arrived_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        self.arrived_at = arrived_at;
        self
    }

    pub fn org_id(mut self, org_id: String) -> Self {
        self.org_id = org_id;
        self
    }

    pub fn proj_id(mut self, proj_id: Option<String>) -> Self {
        self.proj_id = proj_id;
        self
    }

    pub fn session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    pub fn anon_id(mut self, anon_id: String) -> Self {
        self.anon_id = anon_id;
        self
    }

    pub fn actor_id(mut self, actor_id: Option<String>) -> Self {
        self.actor_id = actor_id;
        self
    }

    pub fn source(mut self, source: Option<String>) -> Self {
        self.source = source;
        self
    }

    pub fn correlation_id(mut self, correlation_id: Option<String>) -> Self {
        self.correlation_id = correlation_id;
        self
    }

    pub fn trace_id(mut self, trace_id: Option<String>) -> Self {
        self.trace_id = trace_id;
        self
    }

    pub fn authenticated(mut self, authenticated: Option<bool>) -> Self {
        self.authenticated = authenticated;
        self
    }

    pub fn properties(mut self, properties: Option<serde_json::Value>) -> Self {
        self.properties = properties;
        self
    }

    pub fn system_properties(mut self, system_properties: Option<SystemProperties>) -> Self {
        self.system_properties = system_properties;
        self
    }

    pub fn build(self) -> CanonicalEvent {
        CanonicalEvent {
            envelop_version: self.envelop_version,
            id: self.id.unwrap_or_else(uuid::Uuid::new_v4),
            name: self.name,
            occured_at: self.occured_at.unwrap_or_else(chrono::Utc::now),
            arrived_at: self.arrived_at,
            org_id: self.org_id,
            proj_id: self.proj_id,
            session_id: self.session_id,
            anon_id: self.anon_id,
            actor_id: self.actor_id,
            source: self.source,
            correlation_id: self.correlation_id,
            trace_id: self.trace_id,
            authenticated: self.authenticated,
            properties: self.properties,
            system_properties: self.system_properties,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_arrived_at_as_utc_nanosecond_precision() {
        let arrived_at = chrono::DateTime::parse_from_rfc3339("2026-08-26T10:11:12.123Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let canonical_event = CanonicalEvent::builder()
            .arrived_at(Some(arrived_at))
            .build();
        let payload = serde_json::to_value(canonical_event).unwrap();

        assert_eq!(
            payload
                .get("arrived_at")
                .and_then(serde_json::Value::as_str),
            Some("2026-08-26T10:11:12.123000000Z")
        );
    }

    #[test]
    fn serializes_ip_address_as_snake_case_system_property() {
        let canonical_event = CanonicalEvent::builder()
            .system_properties(Some(
                SystemProperties::builder()
                    .ip_address(Some("203.0.113.10".into()))
                    .build(),
            ))
            .build();
        let payload = serde_json::to_value(canonical_event).unwrap();
        let system_properties = payload
            .get("system_properties")
            .and_then(serde_json::Value::as_object)
            .unwrap();

        assert_eq!(
            system_properties
                .get("ip_address")
                .and_then(serde_json::Value::as_str),
            Some("203.0.113.10")
        );
        assert!(!system_properties.contains_key("ipAddress"));
    }

    #[test]
    fn round_trips_through_json() {
        let arrived_at = chrono::DateTime::parse_from_rfc3339("2026-08-26T10:11:12.123456789Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let canonical_event = CanonicalEvent::builder()
            .name("payment_initiated".into())
            .tenant_id("merchant-1".into())
            .anon_id("anon-42".into())
            .arrived_at(Some(arrived_at))
            .properties(Some(serde_json::json!({ "amount": 100 })))
            .system_properties(Some(
                SystemProperties::builder()
                    .geo(Some(Geo {
                        country: Some("IN".into()),
                    }))
                    .build(),
            ))
            .build();
        let payload = serde_json::to_vec(&canonical_event).unwrap();

        let decoded: CanonicalEvent = serde_json::from_slice(&payload).unwrap();

        assert_eq!(decoded.id(), canonical_event.id());
        assert_eq!(decoded.arrived_at, Some(arrived_at));
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), payload);
    }

    #[test]
    fn deserializes_without_optional_fields() {
        let decoded: CanonicalEvent = serde_json::from_str(
            r#"{"envelop_version":"1.0","id":"0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c","name":"checkout_viewed","tenant_id":"merchant-1","anon_id":"anon-42","occured_at":"2026-09-02T10:29:00Z"}"#,
        )
        .unwrap();

        assert_eq!(
            decoded.id().to_string(),
            "0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c"
        );
        assert!(decoded.arrived_at.is_none());
    }
}
