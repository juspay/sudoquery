//! End-to-end tests against the local stack:
//!
//! ```text
//! docker compose -f tests/docker-compose.yml up -d redpanda opensearch
//! cargo test -p sink-opensearch -- --ignored
//! ```
//!
//! `KAFKA_BOOTSTRAP_SERVERS`, `OPENSEARCH_URL` and `OPENSEARCH_CONTAINER`
//! override the defaults (`localhost:19092`, `http://localhost:9200`,
//! `opensearch`). Each test uses its own topics, index and consumer group.

use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use canonical_event::CanonicalEvent;
use futures::future::join_all;
use rdkafka::admin::{AdminClient, AdminOptions, NewTopic, TopicReplication};
use rdkafka::client::DefaultClientContext;
use rdkafka::consumer::{BaseConsumer, Consumer, StreamConsumer};
use rdkafka::message::{Headers, Message, OwnedMessage};
use rdkafka::producer::{FutureProducer, FutureRecord};
use rdkafka::util::Timeout;
use rdkafka::{ClientConfig, Offset, TopicPartitionList};
use serde_json::{Value, json};
use sink_opensearch::cac::Cac;
use sink_opensearch::health::Health;
use sink_opensearch::{Config, Drained};
use tokio::sync::{Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

const PARTITIONS: i32 = 3;
const WAIT: Duration = Duration::from_secs(90);

/// Tests run one at a time: the outage test pauses OpenSearch for everyone.
async fn serial() -> MutexGuard<'static, ()> {
    static SERIAL: OnceLock<Mutex<()>> = OnceLock::new();
    SERIAL.get_or_init(|| Mutex::new(())).lock().await
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_owned())
}

struct Stack {
    kafka: String,
    opensearch: String,
    http: reqwest::Client,
}

/// Names that belong to one test.
struct Names {
    topic: String,
    dlq: String,
    index: String,
    group: String,
}

impl Names {
    fn unique() -> Self {
        let id = uuid::Uuid::new_v4().simple().to_string();
        Self {
            topic: format!("it-{id}"),
            dlq: format!("it-{id}.dlq"),
            index: format!("it-{id}"),
            group: format!("it-{id}"),
        }
    }
}

impl Stack {
    fn from_env() -> Self {
        Self {
            kafka: env_or("KAFKA_BOOTSTRAP_SERVERS", "localhost:19092"),
            opensearch: env_or("OPENSEARCH_URL", "http://localhost:9200"),
            http: reqwest::Client::new(),
        }
    }

    fn client_config(&self) -> ClientConfig {
        let mut config = ClientConfig::new();
        config.set("bootstrap.servers", &self.kafka);
        config
    }

    /// A sink reading `names.topic` into `names.index`.
    async fn sink(&self, names: &Names, max_docs: usize) -> SinkSettings {
        self.sink_with(names, max_docs, &names.index, "").await
    }

    /// A sink with its own CAC file: `index` is the default
    /// `opensearch.index`, and `overrides` is appended as-is.
    async fn sink_with(
        &self,
        names: &Names,
        max_docs: usize,
        index: &str,
        overrides: &str,
    ) -> SinkSettings {
        let cac_file = format!(
            r#"
[default-configs]
"kafka.topics" = {{ value = ["{topic}"], schema = {{ type = "array" }} }}
"kafka.group_id" = {{ value = "{group}", schema = {{ type = "string" }} }}
"kafka.client_config" = {{ value = {{ "bootstrap.servers" = "{kafka}" }}, schema = {{ type = "object" }} }}
"opensearch.url" = {{ value = "{opensearch}", schema = {{ type = "string" }} }}
"opensearch.index" = {{ value = "{index}", schema = {{ type = "string" }} }}
"opensearch.request_timeout_ms" = {{ value = 2000, schema = {{ type = "integer" }} }}
"batch.max_docs" = {{ value = {max_docs}, schema = {{ type = "integer" }} }}
"batch.linger_ms" = {{ value = 100, schema = {{ type = "integer" }} }}
"retry.initial_backoff_ms" = {{ value = 50, schema = {{ type = "integer" }} }}
"retry.max_backoff_ms" = {{ value = 1000, schema = {{ type = "integer" }} }}
"dlq.topic" = {{ value = "{dlq}", schema = {{ type = "string" }} }}
"commit.interval_ms" = {{ value = 500, schema = {{ type = "integer" }} }}
"shutdown.grace_ms" = {{ value = 10000, schema = {{ type = "integer" }} }}

[dimensions]
tenant_id = {{ position = 1, schema = {{ type = "string" }} }}
workspace_id = {{ position = 2, schema = {{ type = "string" }} }}
{overrides}
"#,
            topic = names.topic,
            group = names.group,
            kafka = self.kafka,
            opensearch = self.opensearch,
            dlq = names.dlq,
        );
        let path = std::env::temp_dir().join(format!(
            "sink-opensearch-it-{}.toml",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, cac_file).unwrap();

        let cac = Cac::load(&path).await.expect("valid test CAC file");
        let config = Config::load(&cac).await.expect("valid test config");
        SinkSettings { cac, config, path }
    }

    async fn create_topics(&self, names: &Names) {
        let admin: AdminClient<DefaultClientContext> = self.client_config().create().unwrap();
        let topics = [
            NewTopic::new(&names.topic, PARTITIONS, TopicReplication::Fixed(1)),
            NewTopic::new(&names.dlq, 1, TopicReplication::Fixed(1)),
        ];
        for result in admin
            .create_topics(&topics, &AdminOptions::new())
            .await
            .unwrap()
        {
            result.unwrap();
        }
    }

    async fn produce(&self, topic: &str, records: &[(String, Vec<u8>)]) {
        let producer: FutureProducer = self.client_config().create().unwrap();
        let deliveries = records.iter().map(|(key, payload)| {
            producer.send(
                FutureRecord::to(topic).key(key).payload(payload),
                Timeout::After(Duration::from_secs(10)),
            )
        });
        for delivery in join_all(deliveries).await {
            delivery.unwrap();
        }
    }

    async fn create_index(&self, index: &str, body: Value) {
        let response = self
            .http
            .put(format!("{}/{index}", self.opensearch))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
    }

    async fn count(&self, index: &str) -> u64 {
        let refresh = self
            .http
            .post(format!("{}/{index}/_refresh", self.opensearch))
            .send()
            .await;
        if refresh.is_err() {
            return 0;
        }
        let Ok(response) = self
            .http
            .get(format!("{}/{index}/_count", self.opensearch))
            .send()
            .await
        else {
            return 0;
        };
        if !response.status().is_success() {
            return 0;
        }
        let body: Value = response.json().await.unwrap();
        body["count"].as_u64().unwrap()
    }

    /// Waits until the index holds at least `expected` documents, and fails if
    /// it holds more: extra documents would mean duplicates.
    async fn wait_for_count(&self, index: &str, expected: u64) {
        let deadline = Instant::now() + WAIT;
        let mut count = 0;
        while Instant::now() < deadline {
            count = self.count(index).await;
            if count >= expected {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        assert_eq!(count, expected, "documents in `{index}`");
    }

    async fn document(&self, index: &str, id: &str) -> Value {
        let response = self
            .http
            .get(format!("{}/{index}/_doc/{id}", self.opensearch))
            .send()
            .await
            .unwrap();
        assert!(response.status().is_success(), "document {id} not found");
        response.json().await.unwrap()
    }

    /// Committed offsets for the group, next to each partition's end offset.
    async fn committed_and_end_offsets(&self, names: &Names) -> (Vec<i64>, Vec<i64>) {
        let mut config = self.client_config();
        config.set("group.id", &names.group);
        let topic = names.topic.clone();

        tokio::task::spawn_blocking(move || {
            let consumer: BaseConsumer = config.create().unwrap();
            let mut partitions = TopicPartitionList::new();
            for partition in 0..PARTITIONS {
                partitions.add_partition(&topic, partition);
            }
            let committed = consumer
                .committed_offsets(partitions, Duration::from_secs(10))
                .unwrap();

            let committed = (0..PARTITIONS)
                .map(|partition| {
                    match committed
                        .find_partition(&topic, partition)
                        .map(|element| element.offset())
                    {
                        Some(Offset::Offset(offset)) => offset,
                        _ => -1,
                    }
                })
                .collect();
            let ends = (0..PARTITIONS)
                .map(|partition| {
                    consumer
                        .fetch_watermarks(&topic, partition, Duration::from_secs(10))
                        .unwrap()
                        .1
                })
                .collect();
            (committed, ends)
        })
        .await
        .unwrap()
    }

    /// Deletes the test's index and topics. Only called when a test passes, so
    /// a failure leaves its data behind to inspect.
    async fn clean_up(&self, names: &Names) {
        self.delete_index(&names.index).await;
        let admin: AdminClient<DefaultClientContext> = self.client_config().create().unwrap();
        let _ = admin
            .delete_topics(&[&names.topic, &names.dlq], &AdminOptions::new())
            .await;
    }

    /// Installs the repo's `index-template.json`, pointed at `pattern`
    /// instead of `events-*`.
    async fn install_index_template(&self, name: &str, pattern: &str) {
        let raw =
            std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/index-template.json"))
                .unwrap();
        let mut template: Value = serde_json::from_str(&raw).unwrap();
        template["index_patterns"] = json!([pattern]);
        let response = self
            .http
            .put(format!("{}/_index_template/{name}", self.opensearch))
            .json(&template)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
    }

    async fn get(&self, path: &str) -> Value {
        self.http
            .get(format!("{}/{path}", self.opensearch))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn search(&self, index: &str, body: Value) -> Value {
        self.http
            .post(format!("{}/{index}/_search", self.opensearch))
            .json(&body)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    async fn search_hits(&self, index: &str, query: Value) -> u64 {
        let body = self.search(index, json!({ "query": query })).await;
        body["hits"]["total"]["value"].as_u64().unwrap()
    }

    async fn delete_index(&self, index: &str) {
        let _ = self
            .http
            .delete(format!("{}/{index}", self.opensearch))
            .send()
            .await;
    }

    async fn read_topic(&self, topic: &str, expected: usize) -> Vec<OwnedMessage> {
        let consumer: StreamConsumer = self
            .client_config()
            .set(
                "group.id",
                format!("it-reader-{}", uuid::Uuid::new_v4().simple()),
            )
            .set("auto.offset.reset", "earliest")
            .create()
            .unwrap();
        consumer.subscribe(&[topic]).unwrap();

        let deadline = Instant::now() + WAIT;
        let mut messages = Vec::new();
        while messages.len() < expected {
            match tokio::time::timeout_at(deadline, consumer.recv()).await {
                Ok(Ok(message)) => messages.push(message.detach()),
                Ok(Err(error)) => panic!("reading `{topic}` failed: {error}"),
                Err(_) => panic!(
                    "got {} of {expected} messages from `{topic}`",
                    messages.len()
                ),
            }
        }
        messages
    }
}

struct SinkSettings {
    cac: Cac,
    config: Config,
    path: PathBuf,
}

struct RunningSink {
    shutdown: CancellationToken,
    handle: JoinHandle<Result<Drained, sink_opensearch::Error>>,
    cac: Cac,
    path: PathBuf,
}

impl RunningSink {
    fn start(settings: SinkSettings) -> Self {
        let SinkSettings { cac, config, path } = settings;
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(sink_opensearch::run(
            config,
            cac.clone(),
            Arc::new(Health::default()),
            shutdown.clone(),
        ));
        Self {
            shutdown,
            handle,
            cac,
            path,
        }
    }

    async fn stop(self) -> Drained {
        self.shutdown.cancel();
        let drained = self.handle.await.unwrap().unwrap();
        self.cac.close().await;
        let _ = std::fs::remove_file(&self.path);
        drained
    }

    /// Stops the sink without a graceful shutdown, like a crash.
    async fn kill(self) {
        self.handle.abort();
        let _ = self.handle.await;
        self.cac.close().await;
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Unpauses the OpenSearch container even if the test fails while it's paused.
struct PausedContainer(String);

impl PausedContainer {
    fn pause(name: String) -> Self {
        docker(&["pause", &name]);
        Self(name)
    }
}

impl Drop for PausedContainer {
    fn drop(&mut self) {
        docker(&["unpause", &self.0]);
    }
}

fn docker(args: &[&str]) {
    let status = Command::new("docker").args(args).status().unwrap();
    assert!(status.success(), "docker {args:?} failed");
}

/// Canonical events keyed by `anon_id`. Returns (key, payload, event id).
fn events(count: usize) -> Vec<(String, Vec<u8>, String)> {
    (0..count)
        .map(|i| {
            let event = CanonicalEvent::builder()
                .name("payment_initiated".into())
                .tenant_id("merchant-1".into())
                .anon_id(format!("anon-{}", i % 50))
                .properties(Some(json!({ "amount": i })))
                .build();
            let key = format!("anon-{}", i % 50);
            (
                key,
                serde_json::to_vec(&event).unwrap(),
                event.id().to_string(),
            )
        })
        .collect()
}

fn records(events: &[(String, Vec<u8>, String)]) -> Vec<(String, Vec<u8>)> {
    events
        .iter()
        .map(|(key, payload, _)| (key.clone(), payload.clone()))
        .collect()
}

fn header(message: &OwnedMessage, key: &str) -> Option<String> {
    message
        .headers()?
        .iter()
        .find(|header| header.key == key)
        .and_then(|header| header.value)
        .map(|value| String::from_utf8_lossy(value).into_owned())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn indexes_every_event_once_and_commits_all_offsets() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    let events = events(1000);
    stack.produce(&names.topic, &records(&events)).await;

    let sink = RunningSink::start(stack.sink(&names, 100).await);
    stack.wait_for_count(&names.index, 1000).await;

    let (_, payload, id) = &events[42];
    let document = stack.document(&names.index, id).await;
    let original: Value = serde_json::from_slice(payload).unwrap();
    assert_eq!(document["_source"], original);

    assert_eq!(sink.stop().await, Drained::Complete);
    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(committed, ends);
    assert_eq!(ends.iter().sum::<i64>(), 1000);
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn a_crash_mid_stream_loses_nothing_and_duplicates_nothing() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    stack.produce(&names.topic, &records(&events(3000))).await;

    let first = RunningSink::start(stack.sink(&names, 20).await);
    let deadline = Instant::now() + WAIT;
    while stack.count(&names.index).await < 300 {
        assert!(Instant::now() < deadline, "the first run made no progress");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    first.kill().await;
    assert!(
        stack.count(&names.index).await < 3000,
        "the first run finished before it was killed; the test proves nothing"
    );

    let second = RunningSink::start(stack.sink(&names, 100).await);
    stack.wait_for_count(&names.index, 3000).await;
    assert_eq!(second.stop().await, Drained::Complete);

    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(committed, ends);
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn documents_that_cannot_be_indexed_go_to_the_dlq() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    stack
        .create_index(
            &names.index,
            json!({"mappings": {"properties": {"properties": {"properties": {"amount": {"type": "integer"}}}}}}),
        )
        .await;

    let mut records = Vec::new();
    let mut bad_payloads = HashSet::new();
    for i in 0..100 {
        let amount = if i % 10 == 0 {
            json!("not-a-number")
        } else {
            json!(i)
        };
        let event = CanonicalEvent::builder()
            .name("payment_initiated".into())
            .tenant_id("merchant-1".into())
            .anon_id(format!("anon-{i}"))
            .properties(Some(json!({ "amount": amount })))
            .build();
        let payload = serde_json::to_vec(&event).unwrap();
        if i % 10 == 0 {
            bad_payloads.insert(payload.clone());
        }
        records.push((format!("anon-{i}"), payload));
    }
    records.push(("garbage".into(), b"this is not json".to_vec()));
    stack.produce(&names.topic, &records).await;

    let sink = RunningSink::start(stack.sink(&names, 100).await);
    stack.wait_for_count(&names.index, 90).await;
    let letters = stack.read_topic(&names.dlq, 11).await;

    let mut rejected = 0;
    let mut undecodable = 0;
    for letter in &letters {
        assert_eq!(
            header(letter, "dlq.source.topic").as_deref(),
            Some(names.topic.as_str())
        );
        assert!(header(letter, "dlq.source.partition").is_some());
        assert!(header(letter, "dlq.source.offset").is_some());
        assert!(header(letter, "dlq.failed_at").is_some());
        let payload = letter.payload().unwrap().to_vec();
        match header(letter, "dlq.error.class").as_deref() {
            Some("rejected") => {
                rejected += 1;
                assert_eq!(header(letter, "dlq.error.status").as_deref(), Some("400"));
                assert!(
                    header(letter, "dlq.error.reason")
                        .unwrap()
                        .contains("mapper_parsing_exception")
                );
                assert!(
                    bad_payloads.contains(&payload),
                    "DLQ payload must be the original bytes"
                );
            }
            Some("decode") => {
                undecodable += 1;
                assert_eq!(payload, b"this is not json");
                assert_eq!(letter.key(), Some(&b"garbage"[..]));
            }
            other => panic!("unexpected error class {other:?}"),
        }
    }
    assert_eq!((rejected, undecodable), (10, 1));

    assert_eq!(sink.stop().await, Drained::Complete);
    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(
        committed, ends,
        "offsets advance past dead-lettered records"
    );
    assert_eq!(stack.count(&names.index).await, 90);
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn an_opensearch_outage_blocks_writes_and_then_recovers() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    stack.produce(&names.topic, &records(&events(500))).await;

    let paused = PausedContainer::pause(env_or("OPENSEARCH_CONTAINER", "opensearch"));
    let sink = RunningSink::start(stack.sink(&names, 50).await);
    tokio::time::sleep(Duration::from_secs(8)).await;
    stack.produce(&names.topic, &records(&events(500))).await;
    tokio::time::sleep(Duration::from_secs(4)).await;
    drop(paused);

    stack.wait_for_count(&names.index, 1000).await;
    assert_eq!(sink.stop().await, Drained::Complete);
    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(committed, ends);
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn each_tenant_gets_its_own_index() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;

    let mut records = Vec::new();
    for (tenant, count) in [("merchant-a", 50), ("merchant-b", 30), ("Merchant-C", 5)] {
        for i in 0..count {
            let event = CanonicalEvent::builder()
                .name("payment_initiated".into())
                .tenant_id(tenant.into())
                .anon_id(format!("anon-{i}"))
                .build();
            records.push((format!("anon-{i}"), serde_json::to_vec(&event).unwrap()));
        }
    }
    stack.produce(&names.topic, &records).await;

    // Every tenant gets `<index>-<tenant>`, except merchant-b, which CAC
    // moves to a dedicated index.
    let template = format!("{}-{{tenant_id}}", names.index);
    let merchant_a = format!("{}-merchant-a", names.index);
    let merchant_b = format!("{}-merchant-b-dedicated", names.index);
    let overrides = format!(
        r#"
[[overrides]]
_context_ = {{ tenant_id = "merchant-b" }}
"opensearch.index" = "{merchant_b}"
"#
    );
    let sink = RunningSink::start(stack.sink_with(&names, 100, &template, &overrides).await);
    stack.wait_for_count(&merchant_a, 50).await;
    stack.wait_for_count(&merchant_b, 30).await;
    assert_eq!(
        stack.count(&format!("{}-merchant-b", names.index)).await,
        0,
        "merchant-b must only be in its dedicated index"
    );

    // Uppercase can't be part of an index name, and the sink never rewrites
    // tenant IDs, so these go to the DLQ.
    let letters = stack.read_topic(&names.dlq, 5).await;
    for letter in &letters {
        assert_eq!(
            header(letter, "dlq.error.class").as_deref(),
            Some("invalid_tenant")
        );
    }

    assert_eq!(sink.stop().await, Drained::Complete);
    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(committed, ends);
    stack.delete_index(&merchant_a).await;
    stack.delete_index(&merchant_b).await;
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn properties_can_change_shape_without_rejections() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    stack
        .install_index_template(&names.index, &format!("{}-*", names.index))
        .await;

    // The same property arrives as a number, a string and an object, and new
    // keys keep appearing. With dynamic mapping, most of these would be
    // rejected after the first one.
    let shapes = [
        json!({ "amount": 100, "plan": "premium" }),
        json!({ "amount": "100 INR", "plan": "basic" }),
        json!({ "amount": { "value": 100, "currency": "INR" } }),
        json!({ "cart": { "items": 3, "coupon": "SAVE10" }, "plan": "premium" }),
    ];
    let mut records = Vec::new();
    for i in 0..40 {
        let event = CanonicalEvent::builder()
            .name("checkout_viewed".into())
            .tenant_id("merchant-a".into())
            .anon_id(format!("anon-{i}"))
            .properties(Some(shapes[i % shapes.len()].clone()))
            .build();
        records.push((format!("anon-{i}"), serde_json::to_vec(&event).unwrap()));
    }
    stack.produce(&names.topic, &records).await;

    let template = format!("{}-{{tenant_id}}", names.index);
    let index = format!("{}-merchant-a", names.index);
    let sink = RunningSink::start(stack.sink_with(&names, 100, &template, "").await);
    stack.wait_for_count(&index, 40).await;
    assert_eq!(sink.stop().await, Drained::Complete);

    let mapping = stack.get(&format!("{index}/_mapping")).await;
    let fields = &mapping[&index]["mappings"]["properties"];
    assert_eq!(fields["properties"]["type"], "flat_object");
    assert_eq!(fields["tenant_id"]["type"], "keyword");
    assert_eq!(fields["occured_at"]["type"], "date");

    let premium = json!({ "term": { "properties.plan": "premium" } });
    assert_eq!(stack.search_hits(&index, premium).await, 20);
    let coupon = json!({ "term": { "properties.cart.coupon": "SAVE10" } });
    assert_eq!(stack.search_hits(&index, coupon).await, 10);

    let (committed, ends) = stack.committed_and_end_offsets(&names).await;
    assert_eq!(committed, ends);
    let _ = stack
        .http
        .delete(format!(
            "{}/_index_template/{}",
            stack.opensearch, names.index
        ))
        .send()
        .await;
    stack.delete_index(&index).await;
    stack.clean_up(&names).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "needs the local stack: docker compose -f tests/docker-compose.yml up -d redpanda opensearch"]
async fn events_are_timed_by_when_they_happened() {
    let _serial = serial().await;
    let stack = Stack::from_env();
    let names = Names::unique();
    stack.create_topics(&names).await;
    stack
        .install_index_template(&names.index, &format!("{}-*", names.index))
        .await;

    // Sent in a different order from when they happened, the way a phone that
    // was offline delivers old events late. arrived_at is the collector's
    // nanosecond format.
    let at = |time: &str| {
        chrono::DateTime::parse_from_rfc3339(time)
            .unwrap()
            .with_timezone(&chrono::Utc)
    };
    let events = [
        (
            "checkout",
            "2026-09-28T11:00:00Z",
            "2026-09-28T11:00:00.100000000Z",
        ),
        (
            "login",
            "2026-09-28T09:00:00Z",
            "2026-09-28T12:00:00.123456789Z",
        ),
        (
            "browse",
            "2026-09-28T10:00:00Z",
            "2026-09-28T12:00:00.987654321Z",
        ),
    ];
    let mut records = Vec::new();
    for (name, occured_at, arrived_at) in events {
        let event = CanonicalEvent::builder()
            .name(name.into())
            .tenant_id("merchant-a".into())
            .anon_id("anon-1".into())
            .occured_at(at(occured_at))
            .arrived_at(Some(at(arrived_at)))
            .build();
        records.push(("anon-1".to_owned(), serde_json::to_vec(&event).unwrap()));
    }
    stack.produce(&names.topic, &records).await;

    let template = format!("{}-{{tenant_id}}", names.index);
    let index = format!("{}-merchant-a", names.index);
    let sink = RunningSink::start(stack.sink_with(&names, 100, &template, "").await);
    stack.wait_for_count(&index, 3).await;
    assert_eq!(sink.stop().await, Drained::Complete);

    let sorted = stack
        .search(&index, json!({ "sort": [{ "@timestamp": "asc" }] }))
        .await;
    let order: Vec<&str> = sorted["hits"]["hits"]
        .as_array()
        .unwrap()
        .iter()
        .map(|hit| hit["_source"]["name"].as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["login", "browse", "checkout"]);

    let after_login = json!({ "range": { "@timestamp": { "gte": "2026-09-28T09:30:00Z" } } });
    assert_eq!(stack.search_hits(&index, after_login).await, 2);

    let _ = stack
        .http
        .delete(format!(
            "{}/_index_template/{}",
            stack.opensearch, names.index
        ))
        .send()
        .await;
    stack.delete_index(&index).await;
    stack.clean_up(&names).await;
}
