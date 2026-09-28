//! Dead letter queue producer.
//!
//! Dead letters keep the original key and payload bytes untouched; where they
//! came from and why they failed travels in headers.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::SecondsFormat;
use rdkafka::ClientConfig;
use rdkafka::error::{KafkaError, KafkaResult};
use rdkafka::message::{Header, OwnedHeaders};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;

use super::offsets::TopicPartition;
use super::write_task::DeadLetter;

/// How long `send` waits for room in the producer's local queue.
const QUEUE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
pub struct Dlq {
    producer: FutureProducer,
    topic: Arc<str>,
}

impl Dlq {
    pub fn new(client_config: &HashMap<String, String>, topic: &str) -> KafkaResult<Self> {
        let mut config = ClientConfig::new();
        for (key, value) in client_config {
            config.set(key, value);
        }
        // Idempotence implies acks=all and prevents duplicates on internal retries.
        config.set("enable.idempotence", "true");

        Ok(Self {
            producer: config.create()?,
            topic: Arc::from(topic),
        })
    }

    /// Resolves once Kafka has acknowledged the dead letter.
    pub async fn send(
        &self,
        source: &TopicPartition,
        letter: &DeadLetter,
    ) -> Result<(), KafkaError> {
        let partition = source.partition.to_string();
        let offset = letter.offset.to_string();
        let attempts = letter.attempts.to_string();
        let failed_at = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let status = letter.rejection.status.map(|status| status.to_string());

        let mut headers = OwnedHeaders::new()
            .insert(header("dlq.source.topic", &source.topic))
            .insert(header("dlq.source.partition", &partition))
            .insert(header("dlq.source.offset", &offset))
            .insert(header("dlq.error.class", letter.rejection.class))
            .insert(header("dlq.error.reason", &letter.rejection.reason))
            .insert(header("dlq.attempts", &attempts))
            .insert(header("dlq.failed_at", &failed_at));
        if let Some(status) = &status {
            headers = headers.insert(header("dlq.error.status", status));
        }

        let mut record = FutureRecord::<[u8], [u8]>::to(&self.topic).headers(headers);
        if let Some(key) = &letter.key {
            record = record.key(key.as_ref());
        }
        if let Some(payload) = &letter.payload {
            record = record.payload(payload.as_ref());
        }

        self.producer
            .send(record, Timeout::After(QUEUE_TIMEOUT))
            .await
            .map(|_| ())
            .map_err(|(error, _)| error)
    }

    /// Blocks until queued dead letters are delivered or the timeout expires.
    pub fn flush(&self, timeout: Duration) -> KafkaResult<()> {
        self.producer.flush(timeout)
    }
}

fn header<'a>(key: &'a str, value: &'a str) -> Header<'a, &'a str> {
    Header {
        key,
        value: Some(value),
    }
}
