use serde::Serialize;
use serde_json::Deserializer;

use crate::canonical_event::CanonicalEvent;
use crate::collector_event::{Batch, CollectorEvent};
use crate::config::Config;
use crate::kafka_connector::push_events_to_kafka;

pub mod auth;
pub mod canonical_event;
pub mod collector_event;
pub mod config;
pub mod enrichment;
pub mod kafka_connector;
pub mod result;

/// The org and project a request is for, from its `x-org-id` and
/// `x-project-id` headers. Every event in the request must carry the same
/// ids: the headers choose the request's config (including its Kafka topic),
/// so an event for another org or project would be published under the
/// wrong one.
pub struct Scope<'a> {
    pub org_id: &'a str,
    pub project_id: &'a str,
}

/// Fails on the first event whose `org_id` / `project_id` differ from `scope`.
fn ensure_in_scope<'e>(
    events: impl IntoIterator<Item = &'e CollectorEvent>,
    scope: &Scope<'_>,
) -> result::Result<()> {
    for (index, event) in events.into_iter().enumerate() {
        if event.org_id != scope.org_id || event.project_id != scope.project_id {
            return Err(result::AppError::ScopeMismatch(format!(
                "event {index} is for org {:?} / project {:?}, but the request's \
                 `x-org-id` / `x-project-id` headers are {:?} / {:?}",
                event.org_id, event.project_id, scope.org_id, scope.project_id
            )));
        }
    }
    Ok(())
}

#[derive(Serialize)]
pub struct CollectionStatus {
    pub filtered: usize,
    pub collected: usize,
    pub total: usize,
}

pub async fn collect_events_batch(
    batch: &str,
    config: &Config,
    scope: &Scope<'_>,
    ip_address: Option<&str>,
) -> result::Result<CollectionStatus> {
    let batch: Batch = serde_json::from_str(batch)?;
    ensure_in_scope(&batch.events, scope)?;
    let total = batch.events.len();

    let canonical_events =
        canonical_event::from_collector_events_batch(batch, Some(config), ip_address);

    push_events_to_kafka(&canonical_events, config).await?;

    Ok(CollectionStatus {
        filtered: total - canonical_events.len(),
        collected: canonical_events.len(),
        total,
    })
}

pub async fn collect_events(
    events: &str,
    config: &Config,
    scope: &Scope<'_>,
    ip_address: Option<&str>,
) -> result::Result<CollectionStatus> {
    collect_events_with_auth(events, config, scope, ip_address, false).await
}

pub async fn collect_events_authenticated(
    events: &str,
    config: &Config,
    scope: &Scope<'_>,
    ip_address: Option<&str>,
) -> result::Result<CollectionStatus> {
    collect_events_with_auth(events, config, scope, ip_address, true).await
}

async fn collect_events_with_auth(
    events: &str,
    config: &Config,
    scope: &Scope<'_>,
    ip_address: Option<&str>,
    authenticated: bool,
) -> result::Result<CollectionStatus> {
    // deserialize the events and collect them
    let stream = Deserializer::from_slice(events.as_bytes()).into_iter::<CollectorEvent>();

    let collector_events: Vec<CollectorEvent> =
        stream.collect::<std::result::Result<Vec<_>, _>>()?;
    ensure_in_scope(&collector_events, scope)?;

    let total = collector_events.len();
    let convert = |event: CollectorEvent| {
        if authenticated {
            canonical_event::from_authenticated_collector_event(event, Some(config), ip_address)
        } else {
            canonical_event::from_collector_event(event, Some(config), ip_address)
        }
    };
    let canonical_events: Vec<CanonicalEvent> =
        collector_events.into_iter().filter_map(convert).collect();

    push_events_to_kafka(&canonical_events, config).await?;

    Ok(CollectionStatus {
        filtered: total - canonical_events.len(),
        collected: canonical_events.len(),
        total,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::net::IpAddr;

    use super::*;
    use crate::kafka_connector::KafkaConnectorConfig;

    const SCOPE: Scope<'static> = Scope {
        org_id: "acme-k3x9qa",
        project_id: "web-shop-9x2k1a",
    };

    fn event_json(org_id: &str, project_id: &str) -> String {
        format!(
            r#"{{"envelop_version":"1.0","id":"{}","name":"page_view","org_id":"{org_id}","project_id":"{project_id}","anon_id":"anon-1","occured_at":"2026-10-09T10:00:00Z"}}"#,
            uuid::Uuid::new_v4()
        )
    }

    /// Never reached in these tests: scope checks fail before publishing.
    fn unreachable_kafka_config() -> Config {
        Config::new(KafkaConnectorConfig {
            topic: "events.generic".to_owned(),
            client_config: HashMap::new(),
        })
    }

    fn events(json: &[String]) -> Vec<CollectorEvent> {
        json.iter()
            .map(|event| serde_json::from_str(event).unwrap())
            .collect()
    }

    #[test]
    fn events_matching_the_headers_are_in_scope() {
        let events = events(&[
            event_json(SCOPE.org_id, SCOPE.project_id),
            event_json(SCOPE.org_id, SCOPE.project_id),
        ]);

        assert!(ensure_in_scope(&events, &SCOPE).is_ok());
    }

    #[test]
    fn events_for_another_org_or_project_are_rejected() {
        for (org_id, project_id) in [
            ("other-org-1x2y3z", SCOPE.project_id),
            (SCOPE.org_id, "other-proj-1x2y3z"),
        ] {
            let events = events(&[
                event_json(SCOPE.org_id, SCOPE.project_id),
                event_json(org_id, project_id),
            ]);

            let error = ensure_in_scope(&events, &SCOPE).unwrap_err();

            let result::AppError::ScopeMismatch(message) = error else {
                panic!("expected a scope mismatch, got {error:?}");
            };
            assert!(message.starts_with("event 1 "), "{message}");
        }
    }

    #[tokio::test]
    async fn collect_events_rejects_out_of_scope_events_before_publishing() {
        let body = format!(
            "{}\n{}",
            event_json(SCOPE.org_id, SCOPE.project_id),
            event_json(SCOPE.org_id, "other-proj-1x2y3z")
        );

        let result = collect_events(&body, &unreachable_kafka_config(), &SCOPE, None).await;

        assert!(matches!(result, Err(result::AppError::ScopeMismatch(_))));
    }

    #[tokio::test]
    async fn collect_events_batch_rejects_out_of_scope_events_before_publishing() {
        let body = format!(
            r#"{{"events":[{},{}]}}"#,
            event_json("other-org-1x2y3z", SCOPE.project_id),
            event_json(SCOPE.org_id, SCOPE.project_id)
        );

        let result = collect_events_batch(&body, &unreachable_kafka_config(), &SCOPE, None).await;

        assert!(matches!(result, Err(result::AppError::ScopeMismatch(_))));
    }

    #[test]
    fn ip2geo_resolves_public_ip() {
        let address: IpAddr = "14.143.32.203".parse().unwrap();
        let country_code = ip2geo::search(&address).unwrap().country;

        assert_eq!(country_code, "IN");
    }
}
