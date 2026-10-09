//! Event ingestion through a running events-collector into Kafka.
//!
//! The collector runs with `KAFKA_TOPIC` set to a per-run pattern
//! (`e2e-collector-<run>.{org_id}.{project_id}`), so every test scope
//! publishes to its own topic. Cases create those topics up front, send
//! events over HTTP and read what landed in Kafka.

use std::collections::HashSet;

use anyhow::{Context, ensure};
use serde_json::{Value, json};

use super::kafka::{Kafka, Record};
use super::server::Service;
use super::{Case, Http, Response, run_cases, select};

const PACKAGE: &str = "event-collector";
/// A public IPv4 address that ip2geo resolves to India.
const CLIENT_IP: &str = "14.143.32.203";
const CLIENT_COUNTRY: &str = "IN";

pub(crate) struct Ctx {
    http: Http,
    kafka: Kafka,
    topic_prefix: String,
}

/// An org/project pair with its own, already created, topic.
struct Scope {
    org_id: String,
    project_id: String,
    topic: String,
}

impl Scope {
    /// The headers that put a request in this scope.
    fn headers(&self) -> Vec<(&str, &str)> {
        vec![
            ("x-org-id", self.org_id.as_str()),
            ("x-project-id", self.project_id.as_str()),
            ("x-forwarded-for", CLIENT_IP),
        ]
    }

    /// A collector event for this scope; returns its id and JSON.
    fn event(&self, name: &str) -> (String, Value) {
        event_for(&self.org_id, &self.project_id, name)
    }
}

impl Ctx {
    /// A fresh org/project pair (valid slugs) whose topic exists.
    async fn scope(&self) -> anyhow::Result<Scope> {
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
        self.scope_in(&format!("e2e-org-{suffix}")).await
    }

    /// A fresh project in `org_id` whose topic exists.
    async fn scope_in(&self, org_id: &str) -> anyhow::Result<Scope> {
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
        let project_id = format!("e2e-proj-{suffix}");
        let topic = format!("{}.{org_id}.{project_id}", self.topic_prefix);
        self.kafka.create_topic(&topic).await?;
        Ok(Scope {
            org_id: org_id.to_owned(),
            project_id,
            topic,
        })
    }

    async fn post_events(
        &self,
        headers: &[(&str, &str)],
        events: &[Value],
    ) -> anyhow::Result<Response> {
        let body = events
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        self.http.post_raw("/v1/events", headers, body).await
    }
}

/// Builds events-collector, starts it against Kafka, runs the selected cases
/// and deletes the topics they created. Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;
    Service::build(PACKAGE);

    let kafka = Kafka::connect().await?;
    let topic_prefix = format!(
        "e2e-collector-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    run_with_kafka(kafka, topic_prefix, &cases).await
}

async fn run_with_kafka(
    kafka: Kafka,
    topic_prefix: String,
    cases: &[&Case<Ctx>],
) -> anyhow::Result<bool> {
    let server = Service::start(
        PACKAGE,
        "SERVER_ADDR",
        // cac.toml is read from the working directory.
        Some(crate::workspace_root()),
        &[
            (
                "KAFKA_BOOTSTRAP_SERVERS",
                kafka.bootstrap_servers().to_owned(),
            ),
            (
                "KAFKA_TOPIC",
                format!("{topic_prefix}.{{org_id}}.{{project_id}}"),
            ),
            ("RUST_LOG", "warn".to_owned()),
        ],
    )
    .await;
    let server = match server {
        Ok(server) => server,
        Err(err) => {
            kafka.delete_created().await?;
            return Err(err);
        }
    };

    let ctx = Ctx {
        http: Http::new(server.base_url()),
        kafka,
        topic_prefix,
    };
    let passed = run_cases(&ctx, cases, |_| {}, || server.print_log_tail()).await;
    ctx.kafka.delete_created().await?;
    Ok(passed)
}

cases![Ctx:
    events_are_published_to_the_project_topic,
    batch_events_carry_batch_system_properties,
    each_project_gets_its_own_topic,
    events_for_another_project_are_rejected,
    requests_without_valid_ids_are_rejected,
    authenticated_endpoint_requires_a_bearer_token,
    health_check_passes_with_a_templated_topic,
];

async fn events_are_published_to_the_project_topic(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let (first_id, first) = scope.event("page_view");
    let (second_id, second) = scope.event("checkout_started");

    let response = ctx.post_events(&scope.headers(), &[first, second]).await?;
    expect_status(&response, 200)?;
    let status = response.json()?;
    ensure!(
        status == json!({ "filtered": 0, "collected": 2, "total": 2 }),
        "unexpected collection status {status}"
    );

    let records = ctx.kafka.read(&scope.topic, 2).await?;
    expect_ids(&records, &[&first_id, &second_id])?;
    for record in &records {
        let event = &record.value;
        ensure!(
            event["org_id"] == scope.org_id.as_str(),
            "wrong org in {event}"
        );
        ensure!(
            event["project_id"] == scope.project_id.as_str(),
            "wrong project in {event}"
        );
        ensure!(
            event["arrived_at"].is_string(),
            "arrived_at not enriched in {event}"
        );
        ensure!(
            event["authenticated"] == false,
            "unexpected authenticated in {event}"
        );
        ensure!(
            event["system_properties"]["ip_address"] == CLIENT_IP,
            "client IP not enriched in {event}"
        );
        ensure!(
            event["system_properties"]["geo"]["country"] == CLIENT_COUNTRY,
            "country not enriched in {event}"
        );
        ensure!(
            record.key.as_deref() == event["anon_id"].as_str(),
            "record key {:?} is not the event's anon_id",
            record.key
        );
    }
    Ok(())
}

async fn batch_events_carry_batch_system_properties(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let (first_id, first) = scope.event("page_view");
    let (second_id, second) = scope.event("page_view");

    let response = ctx
        .http
        .post(
            "/v1/events/batch",
            None,
            &scope.headers(),
            json!({ "events": [first, second], "system_properties": { "timezone": "Asia/Kolkata" } }),
        )
        .await?;
    expect_status(&response, 200)?;

    let records = ctx.kafka.read(&scope.topic, 2).await?;
    expect_ids(&records, &[&first_id, &second_id])?;
    for record in &records {
        let event = &record.value;
        ensure!(
            event["system_properties"]["timezone"] == "Asia/Kolkata",
            "batch timezone missing from {event}"
        );
        ensure!(
            event["system_properties"]["ip_address"] == CLIENT_IP,
            "client IP not enriched in {event}"
        );
    }
    Ok(())
}

async fn each_project_gets_its_own_topic(ctx: &Ctx) -> anyhow::Result<()> {
    let shop = ctx.scope().await?;
    let blog = ctx.scope_in(&shop.org_id).await?;
    let (shop_id, shop_event) = shop.event("purchase");
    let (blog_id, blog_event) = blog.event("comment");

    expect_status(&ctx.post_events(&shop.headers(), &[shop_event]).await?, 200)?;
    expect_status(&ctx.post_events(&blog.headers(), &[blog_event]).await?, 200)?;

    expect_ids(&ctx.kafka.read(&shop.topic, 1).await?, &[&shop_id]).context("shop topic")?;
    expect_ids(&ctx.kafka.read(&blog.topic, 1).await?, &[&blog_id]).context("blog topic")?;
    Ok(())
}

async fn events_for_another_project_are_rejected(ctx: &Ctx) -> anyhow::Result<()> {
    let shop = ctx.scope().await?;
    let blog = ctx.scope_in(&shop.org_id).await?;
    let (_, own) = shop.event("purchase");
    let (_, foreign) = blog.event("comment");

    let response = ctx.post_events(&shop.headers(), &[own, foreign]).await?;
    expect_status(&response, 400)?;
    ensure!(
        response.body.contains("event 1"),
        "error does not name the offending event: {}",
        response.body
    );

    expect_ids(&ctx.kafka.read(&shop.topic, 0).await?, &[]).context("shop topic")?;
    expect_ids(&ctx.kafka.read(&blog.topic, 0).await?, &[]).context("blog topic")?;
    Ok(())
}

async fn requests_without_valid_ids_are_rejected(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let (_, event) = scope.event("page_view");
    let mut without_project = event.clone();
    without_project
        .as_object_mut()
        .unwrap()
        .remove("project_id");

    for (label, headers, body) in [
        (
            "missing x-project-id",
            vec![("x-org-id", scope.org_id.as_str())],
            event.clone(),
        ),
        (
            "invalid x-org-id slug",
            vec![
                ("x-org-id", "Not_A_Slug"),
                ("x-project-id", scope.project_id.as_str()),
            ],
            event.clone(),
        ),
        ("event without project_id", scope.headers(), without_project),
    ] {
        let response = ctx.post_events(&headers, &[body]).await?;
        expect_status(&response, 400).context(label)?;
    }

    expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
}

async fn authenticated_endpoint_requires_a_bearer_token(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let (_, event) = scope.event("page_view");

    let response = ctx
        .http
        .post_raw(
            "/v1/events/authenticated",
            &scope.headers(),
            event.to_string(),
        )
        .await?;
    expect_status(&response, 401)?;
    ensure!(
        response
            .headers
            .get("www-authenticate")
            .is_some_and(|value| value == "Bearer"),
        "missing `WWW-Authenticate: Bearer` on {:?}",
        response.headers
    );

    expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
}

async fn health_check_passes_with_a_templated_topic(ctx: &Ctx) -> anyhow::Result<()> {
    let response = ctx.http.get("/v1/health", None, &[]).await?;
    expect_status(&response, 200)?;
    let body = response.json()?;
    ensure!(body["status"] == "ok", "unexpected health {body}");
    Ok(())
}

// ============ Helpers ============

fn event_for(org_id: &str, project_id: &str, name: &str) -> (String, Value) {
    let id = uuid::Uuid::new_v4().to_string();
    let event = json!({
        "envelop_version": "1.0",
        "id": id,
        "name": name,
        "org_id": org_id,
        "project_id": project_id,
        "anon_id": format!("anon-{}", &id[..8]),
        "occured_at": chrono::Utc::now().to_rfc3339(),
        "properties": { "source": "xtask-e2e" },
    });
    (id, event)
}

/// `records` are exactly the events with `ids`, in any order.
fn expect_ids(records: &[Record], ids: &[&str]) -> anyhow::Result<()> {
    let got: Vec<&str> = records
        .iter()
        .map(|record| record.value["id"].as_str().unwrap_or("<no id>"))
        .collect();
    let expected: HashSet<&str> = ids.iter().copied().collect();
    ensure!(
        got.len() == ids.len() && got.iter().all(|id| expected.contains(id)),
        "expected events {ids:?} on the topic, got {got:?}"
    );
    Ok(())
}

fn expect_status(response: &Response, status: u16) -> anyhow::Result<()> {
    ensure!(
        response.status == status,
        "expected HTTP {status}, got {}: {}",
        response.status,
        response.body
    );
    Ok(())
}
