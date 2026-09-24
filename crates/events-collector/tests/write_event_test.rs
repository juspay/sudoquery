use std::collections::HashMap;

// send canonical event
use event_collector::canonical_event::CanonicalEvent;
use event_collector::config::Config;
use event_collector::kafka_connector::{self, KafkaConnectorConfig};

fn kafka_config() -> KafkaConnectorConfig {
    let mut client_config: HashMap<String, String> = HashMap::new();
    client_config.insert(
        "bootstrap.servers".to_string(),
        "localhost:19092".to_string(),
    );
    client_config.insert("message.timeout.ms".to_string(), "5000".to_string());
    client_config.insert("socket.timeout.ms".to_string(), "5000".to_string());

    KafkaConnectorConfig {
        topic: "events".to_string(),
        client_config,
    }
}

#[test]
fn test_kafka_connection() {
    let connector_conf = kafka_config();
    assert!(kafka_connector::test_connection(&connector_conf).is_ok());
}

#[tokio::test]
async fn test_push_events_to_kafka() {
    let config = Config::new(kafka_config());
    let events = vec![
        CanonicalEvent::builder().build(),
        CanonicalEvent::builder().build(),
    ];

    assert!(
        kafka_connector::push_events_to_kafka(&events, &config)
            .await
            .is_ok()
    );
}
