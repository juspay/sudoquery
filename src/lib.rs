use serde::Serialize;
use serde_json::Deserializer;

use crate::canonical_event::CanonicalEvent;
use crate::collector_event::{Batch, CollectorEvent};
use crate::config::Config;
use crate::kafka_connector::push_events_to_kafka;

pub mod canonical_event;
pub mod collector_event;
pub mod config;
pub mod enrichment;
pub mod kafka_connector;
pub mod result;

#[derive(Serialize)]
pub struct CollectionStatus {
    pub filtered: usize,
    pub collected: usize,
    pub total: usize,
}

pub async fn collect_events_batch(
    batch: &str,
    config: &Config,
    ip_address: Option<&str>,
) -> result::Result<CollectionStatus> {
    let batch: Batch = serde_json::from_str(batch)?;
    let total = batch.events.len();

    let canonical_events =
        CanonicalEvent::from_collector_events_batch(batch, Some(config), ip_address);

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
    ip_address: Option<&str>,
) -> result::Result<CollectionStatus> {
    // deserialize the events and collect them
    let stream = Deserializer::from_slice(events.as_bytes()).into_iter::<CollectorEvent>();

    let collector_events: Vec<CollectorEvent> =
        stream.collect::<std::result::Result<Vec<_>, _>>()?;

    let total = collector_events.len();
    let canonical_events: Vec<CanonicalEvent> = collector_events
        .into_iter()
        .filter_map(|event| CanonicalEvent::from_collector_event(event, Some(config), ip_address))
        .collect();

    push_events_to_kafka(&canonical_events, config).await?;

    Ok(CollectionStatus {
        filtered: total - canonical_events.len(),
        collected: canonical_events.len(),
        total,
    })
}

#[cfg(test)]
mod tests {
    use std::net::IpAddr;

    #[test]
    fn ip2geo_resolves_public_ip() {
        let address: IpAddr = "14.143.32.203".parse().unwrap();
        let country_code = ip2geo::search(&address).unwrap().country;

        assert_eq!(country_code, "IN");
    }
}
