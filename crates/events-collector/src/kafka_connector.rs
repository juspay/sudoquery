use std::collections::{BTreeMap, HashMap};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::Duration;

use futures::future::join_all;
use rdkafka::ClientConfig;
use rdkafka::consumer::{BaseConsumer, Consumer};
use rdkafka::error::KafkaError;
use rdkafka::producer::{FutureProducer, FutureRecord};
use serde::Deserialize;

use crate::canonical_event::CanonicalEvent;
use crate::config::Config;
use crate::result;

/// Placeholders allowed in `kafka_connector.topic`, replaced with the
/// request's ids so each org or project can publish to its own topic
/// (e.g. `events.{org_id}.{project_id}`).
const ORG_ID_PLACEHOLDER: &str = "{org_id}";
const PROJECT_ID_PLACEHOLDER: &str = "{project_id}";

/// Kafka's limit on topic name length.
const MAX_TOPIC_LEN: usize = 249;

#[derive(Deserialize)]
pub struct KafkaConnectorConfig {
    /// Topic name, or a template using `{org_id}` / `{project_id}`.
    pub topic: String,
    pub client_config: HashMap<String, String>,
}

/// `template` with `{org_id}` and `{project_id}` replaced. Fails when the
/// template uses any other `{...}` placeholder or the result is not a legal
/// Kafka topic name.
pub fn render_topic(template: &str, org_id: &str, project_id: &str) -> result::Result<String> {
    let topic = template
        .replace(ORG_ID_PLACEHOLDER, org_id)
        .replace(PROJECT_ID_PLACEHOLDER, project_id);

    if topic.contains(['{', '}']) {
        return Err(result::AppError::Config(format!(
            "`kafka_connector.topic` template {template:?} may only use {ORG_ID_PLACEHOLDER} and \
             {PROJECT_ID_PLACEHOLDER}"
        )));
    }
    let legal = !topic.is_empty()
        && topic.len() <= MAX_TOPIC_LEN
        && topic != "."
        && topic != ".."
        && topic
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
    if !legal {
        return Err(result::AppError::Config(format!(
            "`kafka_connector.topic` {template:?} rendered to {topic:?}, which is not a valid \
             Kafka topic name (1-{MAX_TOPIC_LEN} chars of [a-zA-Z0-9._-])"
        )));
    }
    Ok(topic)
}

/// Whether `topic` still holds placeholders, i.e. it was resolved without a
/// request's ids (the health check) and names no single topic.
fn is_template(topic: &str) -> bool {
    topic.contains(ORG_ID_PLACEHOLDER) || topic.contains(PROJECT_ID_PLACEHOLDER)
}

// test the connection and return Ok or Err
pub fn test_connection(connector_config: &KafkaConnectorConfig) -> Result<(), KafkaError> {
    let mut client_config = ClientConfig::new();
    for (key, value) in &connector_config.client_config {
        client_config.set(key, value);
    }

    let consumer: BaseConsumer = client_config.create()?;

    // A templated topic names no single topic, so only check the brokers.
    let topic = Some(connector_config.topic.as_str()).filter(|topic| !is_template(topic));
    consumer.fetch_metadata(topic, Duration::from_secs(5))?;

    Ok(())
}

/// Producers shared across requests, one per distinct client config.
///
/// Creating a producer starts librdkafka threads and broker connections
/// (~100 ms), far more than sending a request's events, so they are created
/// once and reused. A `client_config` changed in `cac.toml` gets a new entry;
/// configs are few and long-lived, so old entries are simply kept.
static PRODUCERS: LazyLock<Mutex<HashMap<BTreeMap<String, String>, FutureProducer>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The shared producer for `client_config`, created on first use.
fn producer_for(client_config: &HashMap<String, String>) -> Result<FutureProducer, KafkaError> {
    let key: BTreeMap<String, String> = client_config
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let mut producers = PRODUCERS.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(producer) = producers.get(&key) {
        return Ok(producer.clone());
    }

    let mut config = ClientConfig::new();
    for (key, value) in &key {
        config.set(key, value);
    }
    let producer: FutureProducer = config.create()?;
    producers.insert(key, producer.clone());
    Ok(producer)
}

pub async fn push_events_to_kafka(
    events: &Vec<CanonicalEvent>,
    config: &Config,
) -> result::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let producer = producer_for(&config.kafka_connector.client_config)?;

    let topic = config.kafka_connector.topic.as_str();

    let delivery_futures = events.iter().map(|event| {
        let producer = &producer;
        let payload = serde_json::to_vec(event).expect("failed to serialize event");
        let key = event.anon_id.clone();

        async move {
            let record = FutureRecord::to(topic).payload(&payload).key(&key);
            producer.send(record, Duration::from_secs(0)).await
        }
    });

    let results = join_all(delivery_futures).await;

    for (i, delivery_result) in results.into_iter().enumerate() {
        if let Err((err, _)) = delivery_result {
            tracing::error!("failed to deliver event at index {}: {}", i, err);
            return Err(err.into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rdkafka::producer::Producer;

    use super::*;

    #[test]
    fn plain_topics_are_unchanged() {
        assert_eq!(
            render_topic("events.generic", "acme-k3x9qa", "web-shop-9x2k1a").unwrap(),
            "events.generic"
        );
    }

    #[test]
    fn placeholders_are_replaced_with_request_ids() {
        assert_eq!(
            render_topic(
                "events.{org_id}.{project_id}",
                "acme-k3x9qa",
                "web-shop-9x2k1a"
            )
            .unwrap(),
            "events.acme-k3x9qa.web-shop-9x2k1a"
        );
        assert_eq!(
            render_topic("events.{project_id}", "acme-k3x9qa", "web-shop-9x2k1a").unwrap(),
            "events.web-shop-9x2k1a"
        );
        assert_eq!(
            render_topic("{org_id}-{org_id}", "acme-k3x9qa", "web-shop-9x2k1a").unwrap(),
            "acme-k3x9qa-acme-k3x9qa"
        );
    }

    #[test]
    fn unknown_placeholders_are_rejected() {
        for template in ["events.{tenant_id}", "events.{org_id", "events.org_id}"] {
            assert!(
                matches!(
                    render_topic(template, "acme-k3x9qa", "web-shop-9x2k1a"),
                    Err(result::AppError::Config(_))
                ),
                "{template:?} should be rejected"
            );
        }
    }

    #[test]
    fn illegal_topic_names_are_rejected() {
        let too_long = "e".repeat(MAX_TOPIC_LEN + 1);
        for template in [
            "",
            ".",
            "..",
            "events/{org_id}",
            "events {org_id}",
            too_long.as_str(),
        ] {
            assert!(
                matches!(
                    render_topic(template, "acme-k3x9qa", "web-shop-9x2k1a"),
                    Err(result::AppError::Config(_))
                ),
                "{template:?} should be rejected"
            );
        }
    }

    fn client_config(servers: &str) -> HashMap<String, String> {
        HashMap::from([
            ("bootstrap.servers".to_owned(), servers.to_owned()),
            ("message.timeout.ms".to_owned(), "5000".to_owned()),
        ])
    }

    #[test]
    fn producers_are_shared_per_client_config() {
        let first = producer_for(&client_config("producer-cache-a:9092")).unwrap();
        let again = producer_for(&client_config("producer-cache-a:9092")).unwrap();
        let other = producer_for(&client_config("producer-cache-b:9092")).unwrap();

        assert_eq!(first.client().native_ptr(), again.client().native_ptr());
        assert_ne!(first.client().native_ptr(), other.client().native_ptr());
    }

    #[test]
    fn templates_are_detected() {
        assert!(is_template("events.{org_id}"));
        assert!(is_template("events.{project_id}"));
        assert!(!is_template("events.generic"));
    }
}
