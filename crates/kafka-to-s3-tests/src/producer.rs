//! Kafka producer for canonical test events.

use std::time::Duration;

use canonical_event::CanonicalEvent;
use futures::future::join_all;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::types::RDKafkaErrorCode;

use crate::events::event_id;

/// A canonical event delivered to Kafka, with its delivery offset.
#[derive(Clone)]
pub struct ProducedEvent {
    /// The event's canonical `id`.
    pub id: uuid::Uuid,
    /// The Kafka offset the event landed on (partition 0).
    ///
    /// Offsets are production-order markers only: the harness pins partition
    /// 0, so within one archived file the consumption order equals the
    /// offset order. They are never file-name material — archived object
    /// names are content hashes.
    pub offset: i64,
}

/// Creates the test topic on `brokers` if it does not exist yet.
///
/// The topic is created with a single partition and fixed replication
/// factor 1. "Topic already exists" is tolerated because the topic (and
/// the whole cluster) is shared across test runs.
pub async fn ensure_topic(brokers: &str, topic: &str) {
    let admin: AdminClient<DefaultClientContext> = ClientConfig::new()
        .set("bootstrap.servers", brokers)
        .create()
        .unwrap_or_else(|err| {
            panic!(
                "failed to create Kafka admin client for brokers {brokers} (K2S_TEST_KAFKA): {err}"
            )
        });

    let new_topic = NewTopic::new(topic, 1, TopicReplication::Fixed(1));
    let results = admin
        .create_topics(&[new_topic], &AdminOptions::new())
        .await
        .unwrap_or_else(|err| {
            panic!("failed to submit creation of topic {topic} on brokers {brokers}: {err}")
        });

    for result in results {
        match result {
            Ok(_) => {}
            Err((_topic, RDKafkaErrorCode::TopicAlreadyExists)) => {
                // The topic is shared across runs; a previous run or the
                // operator already created it.
            }
            Err((topic, code)) => panic!("failed to create topic {topic}: {code}"),
        }
    }
}

/// Kafka producer for canonical test events, pinned to partition 0.
///
/// Pinning to a single partition makes production order well-defined: within
/// one archived file, the service's consumption order equals the harness's
/// production order.
pub struct EventProducer {
    producer: FutureProducer,
    topic: String,
}

impl EventProducer {
    /// Creates a producer against `brokers` for `topic`.
    pub fn new(brokers: &str, topic: &str) -> EventProducer {
        let producer: FutureProducer = ClientConfig::new()
            .set("bootstrap.servers", brokers)
            .create()
            .unwrap_or_else(|err| {
                panic!(
                    "failed to create Kafka producer for brokers {brokers} (K2S_TEST_KAFKA): {err}"
                )
            });
        EventProducer {
            producer,
            topic: topic.to_string(),
        }
    }

    /// Produces `events` to the configured topic and returns one
    /// [`ProducedEvent`] per input event, in input order.
    ///
    /// Each record is keyed by its event id and pinned to partition 0. The
    /// delivery acknowledgement is awaited for every event, so by the time
    /// this returns, every event is durably in Kafka and the returned
    /// offsets order the events by production order.
    pub async fn produce(&self, events: Vec<CanonicalEvent>) -> Vec<ProducedEvent> {
        let topic = self.topic.as_str();
        let sends = events.into_iter().map(|event| {
            let producer = &self.producer;
            async move {
                let payload = serde_json::to_string(&event)
                    .expect("failed to serialize canonical event to JSON");
                let id = event_id(&event);
                let key = id.to_string();
                let record = FutureRecord::to(topic)
                    .payload(&payload)
                    .key(&key)
                    .partition(0);
                let (partition, offset) = producer
                    .send(record, Duration::from_secs(30))
                    .await
                    .unwrap_or_else(|(err, _message)| {
                        panic!("failed to deliver event {key} to topic {topic}: {err}")
                    });
                assert_eq!(
                    partition, 0,
                    "event {key} was delivered to partition {partition}, but the harness \
                     pins every event to partition 0"
                );
                ProducedEvent { id, offset }
            }
        });
        join_all(sends).await
    }
}
