use std::collections::HashMap;
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

#[derive(Deserialize)]
pub struct KafkaConnectorConfig {
    pub topic: String,
    pub client_config: HashMap<String, String>,
}

// test the connection and return Ok or Err
pub fn test_connection(connector_config: &KafkaConnectorConfig) -> Result<(), KafkaError> {
    let mut client_config = ClientConfig::new();
    for (key, value) in &connector_config.client_config {
        client_config.set(key, value);
    }

    let consumer: BaseConsumer = client_config.create()?;

    consumer.fetch_metadata(Some(&connector_config.topic), Duration::from_secs(5))?;

    Ok(())
}

pub async fn push_events_to_kafka(
    events: &Vec<CanonicalEvent>,
    config: &Config,
) -> result::Result<()> {
    if events.is_empty() {
        return Ok(());
    }

    let mut client_config = ClientConfig::new();
    for (key, value) in &config.kafka_connector.client_config {
        client_config.set(key, value);
    }
    let producer: FutureProducer = client_config.create()?;

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
