use crate::collector_event::{self, Batch, CollectorEvent, EnvelopVersion, Geo};
use crate::config::Config;
use crate::enrichment::{CountryResolution, EnrichmentConfig};
use chrono::SecondsFormat;
use serde::{Serialize, Serializer};
use serde_with::skip_serializing_none;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

#[skip_serializing_none]
#[derive(Serialize)]
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
    pub tenant_id: String,
    pub workspace_id: Option<String>,
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

    pub fn from_collector_event(
        collector_event: CollectorEvent,
        config: Option<&Config>,
        ip_address: Option<&str>,
    ) -> Option<Self> {
        Self::from_collector_event_authenticated(collector_event, false, config, ip_address)
    }

    pub fn from_authenticated_collector_event(
        collector_event: CollectorEvent,
        config: Option<&Config>,
        ip_address: Option<&str>,
    ) -> Option<Self> {
        Self::from_collector_event_authenticated(collector_event, true, config, ip_address)
    }

    fn from_collector_event_authenticated(
        collector_event: CollectorEvent,
        authenticated: bool,
        config: Option<&Config>,
        ip_address: Option<&str>,
    ) -> Option<Self> {
        if config.is_some_and(|config| !config.is_event_allowed(&collector_event)) {
            return None;
        }

        Some(Self::from_collector_event_with_enrichment(
            collector_event,
            authenticated,
            config.and_then(|config| config.enrichment.as_ref()),
            ip_address,
        ))
    }

    pub(crate) fn from_collector_event_with_enrichment(
        collector_event: CollectorEvent,
        authenticated: bool,
        enrichment_config: Option<&EnrichmentConfig>,
        ip_address: Option<&str>,
    ) -> Self {
        Self::from_collector_event_with_system_properties(
            collector_event,
            authenticated,
            None,
            enrichment_config,
            ip_address,
        )
    }

    fn from_collector_event_with_system_properties(
        collector_event: CollectorEvent,
        authenticated: bool,
        system_properties_override: Option<Option<collector_event::SystemProperties>>,
        enrichment_config: Option<&EnrichmentConfig>,
        ip_address: Option<&str>,
    ) -> Self {
        let CollectorEvent {
            envelop_version,
            id,
            name,
            tenant_id,
            workspace_id,
            session_id,
            anon_id,
            actor_id,
            source,
            occured_at,
            properties,
            correlation_id,
            trace_id,
            system_properties,
            ..
        } = collector_event;
        let system_properties = system_properties_override.unwrap_or(system_properties);
        let system_properties = system_properties.map(SystemProperties::from);

        let mut canonical_event = CanonicalEvent {
            envelop_version,
            id,
            name,
            occured_at,
            arrived_at: None,
            tenant_id,
            workspace_id,
            session_id,
            anon_id,
            actor_id,
            source,
            correlation_id,
            trace_id,
            authenticated: Some(authenticated),
            properties,
            system_properties,
        };

        canonical_event.enrich(enrichment_config, ip_address);
        canonical_event
    }

    pub fn from_collector_events_batch(
        batch: Batch,
        config: Option<&Config>,
        ip_address: Option<&str>,
    ) -> Vec<Self> {
        let batch_system_properties = batch.system_properties;

        batch
            .events
            .into_iter()
            .filter_map(|collector_event| {
                if config.is_some_and(|config| !config.is_event_allowed(&collector_event)) {
                    return None;
                }

                Some(Self::from_collector_event_with_system_properties(
                    collector_event,
                    false,
                    Some(batch_system_properties.clone()),
                    config.and_then(|config| config.enrichment.as_ref()),
                    ip_address,
                ))
            })
            .collect()
    }

    fn enrich(&mut self, config: Option<&EnrichmentConfig>, ip_address: Option<&str>) {
        if let Some(enrichment_config) = config {
            if let Some(country_config) = &enrichment_config.country {
                match &country_config.resolution {
                    CountryResolution::IpAddress => {
                        let existing = self
                            .system_properties
                            .as_ref()
                            .and_then(|sp| sp.geo.as_ref())
                            .and_then(|geo| geo.country.as_ref());

                        if country_config.override_existing || existing.is_none() {
                            if let Some(ip_address) = ip_address {
                                if let Ok(addr) = ip_address.parse::<IpAddr>() {
                                    if is_geolocatable_ip(&addr)
                                        && let Some(result) = ip2geo::search(&addr)
                                    {
                                        self.system_properties
                                            .get_or_insert_with(SystemProperties::default)
                                            .geo
                                            .get_or_insert_with(Geo::default)
                                            .country = Some(result.country);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if enrichment_config.arrived_at.enabled
                && (enrichment_config.arrived_at.override_existing || self.arrived_at.is_none())
            {
                self.arrived_at = Some(chrono::Utc::now());
            }

            if let Some(ip_address_config) = &enrichment_config.ip_address {
                let existing = self
                    .system_properties
                    .as_ref()
                    .and_then(|sp| sp.ip_address.as_ref());

                if ip_address_config.override_existing || existing.is_none() {
                    if let Some(ip_address) = ip_address {
                        self.system_properties
                            .get_or_insert_with(SystemProperties::default)
                            .ip_address = Some(ip_address.to_string());
                    }
                }
            }
        }
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

fn is_geolocatable_ip(addr: &IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => is_geolocatable_ipv4(addr),
        IpAddr::V6(addr) => is_geolocatable_ipv6(addr),
    }
}

fn is_geolocatable_ipv4(addr: &Ipv4Addr) -> bool {
    !addr.is_unspecified()
        && !addr.is_loopback()
        && !addr.is_private()
        && !addr.is_link_local()
        && !addr.is_multicast()
        && !addr.is_broadcast()
        && !is_documentation_ipv4(addr)
        && !is_shared_ipv4(addr)
}

fn is_geolocatable_ipv6(addr: &Ipv6Addr) -> bool {
    !addr.is_unspecified()
        && !addr.is_loopback()
        && !addr.is_unique_local()
        && !addr.is_unicast_link_local()
        && !addr.is_multicast()
        && !is_documentation_ipv6(addr)
}

fn is_documentation_ipv4(addr: &Ipv4Addr) -> bool {
    matches!(
        addr.octets(),
        [192, 0, 2, _] | [198, 51, 100, _] | [203, 0, 113, _]
    )
}

fn is_shared_ipv4(addr: &Ipv4Addr) -> bool {
    let [first, second, _, _] = addr.octets();
    first == 100 && (64..=127).contains(&second)
}

fn is_documentation_ipv6(addr: &Ipv6Addr) -> bool {
    let segments = addr.segments();
    segments[0] == 0x2001 && segments[1] == 0x0db8
}

#[skip_serializing_none]
#[derive(Default, Serialize)]
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

impl From<collector_event::SystemProperties> for SystemProperties {
    fn from(props: collector_event::SystemProperties) -> Self {
        SystemProperties {
            geo: props.geo,
            timezone: props.timezone,
            ip_address: props.ip_address,
        }
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
    system_properties: Option<SystemProperties>,
    arrived_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl CanonicalEventBuilder {
    pub fn system_properties(mut self, system_properties: Option<SystemProperties>) -> Self {
        self.system_properties = system_properties;
        self
    }

    pub fn arrived_at(mut self, arrived_at: Option<chrono::DateTime<chrono::Utc>>) -> Self {
        self.arrived_at = arrived_at;
        self
    }

    pub fn build(self) -> CanonicalEvent {
        CanonicalEvent {
            envelop_version: EnvelopVersion::V1,
            id: uuid::Uuid::new_v4(),
            name: String::new(),
            occured_at: chrono::Utc::now(),
            arrived_at: self.arrived_at,
            tenant_id: String::new(),
            workspace_id: None,
            session_id: None,
            anon_id: String::new(),
            actor_id: None,
            source: None,
            correlation_id: None,
            trace_id: None,
            authenticated: None,
            properties: None,
            system_properties: self.system_properties,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrichment::{ArrivedAtConfig, CountryConfig, EnrichmentConfig, IpAddressConfig};

    fn collector_event(
        system_properties: Option<collector_event::SystemProperties>,
    ) -> CollectorEvent {
        CollectorEvent {
            envelop_version: EnvelopVersion::V1,
            id: uuid::Uuid::new_v4(),
            name: "payment_submitted".to_string(),
            tenant_id: "tenant-1".to_string(),
            workspace_id: None,
            session_id: None,
            anon_id: "anon-1".to_string(),
            actor_id: None,
            source: None,
            occured_at: chrono::Utc::now(),
            properties: None,
            correlation_id: None,
            trace_id: None,
            system_properties,
        }
    }

    #[test]
    fn preserves_system_properties_timezone() {
        let event = collector_event(Some(collector_event::SystemProperties {
            geo: None,
            timezone: Some("Asia/Kolkata".to_string()),
            ip_address: None,
        }));

        let canonical_event = CanonicalEvent::from_collector_event(event, None, None).unwrap();

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.timezone),
            Some("Asia/Kolkata".to_string())
        );
    }

    #[test]
    fn enriches_country_from_ip_address_argument() {
        let enrichment_config = EnrichmentConfig::builder()
            .country(
                CountryConfig::builder()
                    .resolution(CountryResolution::IpAddress)
                    .build(),
            )
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(Some(collector_event::SystemProperties {
                geo: None,
                timezone: None,
                ip_address: None,
            })),
            false,
            Some(&enrichment_config),
            Some("14.143.32.203"),
        );

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.geo)
                .and_then(|geo| geo.country),
            ip2geo::search(&"14.143.32.203".parse().unwrap()).map(|geo| geo.country)
        );
    }

    #[test]
    fn does_not_enrich_country_from_loopback_ip_address() {
        let enrichment_config = EnrichmentConfig::builder()
            .country(
                CountryConfig::builder()
                    .resolution(CountryResolution::IpAddress)
                    .build(),
            )
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(Some(collector_event::SystemProperties {
                geo: None,
                timezone: None,
                ip_address: None,
            })),
            false,
            Some(&enrichment_config),
            Some("127.0.0.1"),
        );

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.geo)
                .and_then(|geo| geo.country),
            None
        );
    }

    #[test]
    fn does_not_add_request_ip_address_without_enrichment_config() {
        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(Some(collector_event::SystemProperties {
                geo: None,
                timezone: Some("Asia/Kolkata".to_string()),
                ip_address: None,
            })),
            false,
            None,
            Some("203.0.113.10"),
        );

        let system_properties = canonical_event.system_properties.unwrap();

        assert_eq!(system_properties.ip_address, None);
        assert_eq!(system_properties.timezone, Some("Asia/Kolkata".to_string()));
    }

    #[test]
    fn enriches_request_ip_address_to_system_properties() {
        let enrichment_config = EnrichmentConfig::builder()
            .ip_address(IpAddressConfig::builder().build())
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(None),
            false,
            Some(&enrichment_config),
            Some("203.0.113.10"),
        );

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.ip_address),
            Some("203.0.113.10".to_string())
        );
    }

    #[test]
    fn preserves_existing_ip_address_when_enrichment_does_not_override() {
        let enrichment_config = EnrichmentConfig::builder()
            .ip_address(IpAddressConfig::builder().build())
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(Some(collector_event::SystemProperties {
                geo: None,
                timezone: None,
                ip_address: Some("198.51.100.10".to_string()),
            })),
            false,
            Some(&enrichment_config),
            Some("203.0.113.10"),
        );

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.ip_address),
            Some("198.51.100.10".to_string())
        );
    }

    #[test]
    fn overrides_existing_ip_address_when_enrichment_is_configured_to_override() {
        let enrichment_config = EnrichmentConfig::builder()
            .ip_address(IpAddressConfig::builder().override_existing(true).build())
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(Some(collector_event::SystemProperties {
                geo: None,
                timezone: None,
                ip_address: Some("198.51.100.10".to_string()),
            })),
            false,
            Some(&enrichment_config),
            Some("203.0.113.10"),
        );

        assert_eq!(
            canonical_event
                .system_properties
                .and_then(|props| props.ip_address),
            Some("203.0.113.10".to_string())
        );
    }

    #[test]
    fn serializes_ip_address_as_snake_case_system_property() {
        let enrichment_config = EnrichmentConfig::builder()
            .ip_address(IpAddressConfig::builder().build())
            .build();
        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(None),
            false,
            Some(&enrichment_config),
            Some("203.0.113.10"),
        );
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
    fn enriches_arrived_at_by_default() {
        let enrichment_config = EnrichmentConfig::builder().build();
        let before = chrono::Utc::now();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(None),
            false,
            Some(&enrichment_config),
            None,
        );

        let after = chrono::Utc::now();
        let arrived_at = canonical_event.arrived_at.unwrap();

        assert!(arrived_at >= before);
        assert!(arrived_at <= after);
    }

    #[test]
    fn does_not_enrich_arrived_at_when_disabled() {
        let enrichment_config = EnrichmentConfig::builder()
            .arrived_at(ArrivedAtConfig::builder().enabled(false).build())
            .build();

        let canonical_event = CanonicalEvent::from_collector_event_with_enrichment(
            collector_event(None),
            false,
            Some(&enrichment_config),
            None,
        );

        assert_eq!(canonical_event.arrived_at, None);
    }

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
}
