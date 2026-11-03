//! Event ingestion through a running events-collector into Kafka.
//!
//! The collector runs with `KAFKA_TOPIC` set to a per-run pattern
//! (`e2e-collector-<run>.{workspace_id}`), so every test scope
//! publishes to its own topic. Cases create those topics up front, send
//! events over HTTP and read what landed in Kafka.
//!
//! The authenticated endpoint takes collector keys (`sk_<kid>_<secret>`),
//! which the collector reads from `collector_keys` in a throwaway database
//! on the compose Postgres, as a role that may only read that table.

use std::collections::HashSet;

use anyhow::{Context, ensure};
use serde_json::{Value, json};

use super::kafka::{Kafka, Record};
use super::keys::KeyStore;
use super::server::Service;
use super::{Case, Http, Response, run_cases, select};

const PACKAGE: &str = "event-collector";
/// The suite collector's `COLLECTOR_KEYS_CACHE_SECS`.
const KEY_CACHE_SECS: u64 = 1;
/// A public IPv4 address that ip2geo resolves to India.
const CLIENT_IP: &str = "14.143.32.203";
const CLIENT_COUNTRY: &str = "IN";

pub(crate) struct Ctx {
    http: Http,
    kafka: Kafka,
    keys: KeyStore,
    topic_prefix: String,
}

/// An org/workspace pair with its own, already created, topic.
struct Scope {
    org_id: String,
    workspace_id: String,
    topic: String,
}

impl Scope {
    /// The headers that put a request in this scope.
    fn headers(&self) -> Vec<(&str, &str)> {
        vec![
            ("x-org-id", self.org_id.as_str()),
            ("x-workspace-id", self.workspace_id.as_str()),
            ("x-forwarded-for", CLIENT_IP),
        ]
    }

    /// A collector event for this scope; returns its id and JSON.
    fn event(&self, name: &str) -> (String, Value) {
        event_for(&self.org_id, &self.workspace_id, name)
    }
}

impl Ctx {
    /// A fresh org/workspace pair (valid slugs) whose topic exists.
    async fn scope(&self) -> anyhow::Result<Scope> {
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
        self.scope_in(&format!("e2e_org_{suffix}")).await
    }

    /// A fresh workspace in `org_id` whose topic exists.
    async fn scope_in(&self, org_id: &str) -> anyhow::Result<Scope> {
        let suffix = &uuid::Uuid::new_v4().simple().to_string()[..6];
        let workspace_id = format!("{org_id}-e2e_workspace_{suffix}");
        let topic = format!("{}.{workspace_id}", self.topic_prefix);
        self.kafka.create_topic(&topic).await?;
        Ok(Scope {
            org_id: org_id.to_owned(),
            workspace_id,
            topic,
        })
    }

    async fn post_events(
        &self,
        headers: &[(&str, &str)],
        events: &[Value],
    ) -> anyhow::Result<Response> {
        self.http
            .post_raw("/v1/events", headers, ndjson(events))
            .await
    }

    /// A collector of its own, reading keys from `keys_database_url` (or
    /// with no key store at all).
    async fn collector_with_keys_at(
        &self,
        keys_database_url: Option<&str>,
    ) -> anyhow::Result<Service> {
        let mut env = collector_env(&self.kafka, &self.topic_prefix);
        if let Some(url) = keys_database_url {
            env.push(("COLLECTOR_KEYS_DATABASE_URL", url.to_owned()));
        }
        Service::start(PACKAGE, "SERVER_ADDR", None, &env).await
    }

    async fn post_authenticated(
        &self,
        headers: &[(&str, &str)],
        events: &[Value],
    ) -> anyhow::Result<Response> {
        self.http
            .post_raw("/v1/events/authenticated", headers, ndjson(events))
            .await
    }
}

fn ndjson(events: &[Value]) -> String {
    events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// What every collector in this suite runs with.
fn collector_env(kafka: &Kafka, topic_prefix: &str) -> Vec<(&'static str, String)> {
    vec![
        (
            "COLLECTOR_CONFIG",
            crate::workspace_root()
                .join("crates/events-collector/cac.toml")
                .display()
                .to_string(),
        ),
        (
            "KAFKA_BOOTSTRAP_SERVERS",
            kafka.bootstrap_servers().to_owned(),
        ),
        (
            "KAFKA_TOPIC",
            format!("{topic_prefix}.{{workspace_id}}"),
        ),
        ("RUST_LOG", "warn".to_owned()),
    ]
}

/// Builds events-collector, starts it against Kafka, runs the selected cases
/// and deletes the topics they created. Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;
    Service::build(PACKAGE);

    let kafka = Kafka::connect().await?;
    let keys = KeyStore::create().await?;
    let topic_prefix = format!(
        "e2e-collector-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let ctx = Ctx {
        http: Http::new(String::new()),
        kafka,
        keys,
        topic_prefix,
    };
    run_with_infra(ctx, &cases).await
}

async fn run_with_infra(mut ctx: Ctx, cases: &[&Case<Ctx>]) -> anyhow::Result<bool> {
    let mut env = collector_env(&ctx.kafka, &ctx.topic_prefix);
    env.push((
        "COLLECTOR_KEYS_DATABASE_URL",
        ctx.keys.collector_url.clone(),
    ));
    // Short, so the revocation case needn't wait long for it.
    env.push(("COLLECTOR_KEYS_CACHE_SECS", KEY_CACHE_SECS.to_string()));
    let server = Service::start(PACKAGE, "SERVER_ADDR", None, &env).await;
    let passed = match server {
        Ok(server) => {
            ctx.http = Http::new(server.base_url());
            Ok(run_cases(&ctx, cases, |_| {}, || server.print_log_tail()).await)
        }
        Err(err) => Err(err),
    };
    let topics = ctx.kafka.delete_created().await;
    let keys = ctx.keys.drop().await;
    let passed = passed?;
    topics?;
    keys?;
    Ok(passed)
}

cases![Ctx:
    events_are_published_to_the_workspace_topic,
    batch_events_carry_batch_system_properties,
    each_workspace_gets_its_own_topic,
    events_for_another_workspace_are_rejected,
    requests_without_valid_ids_are_rejected,
    authenticated_endpoint_requires_a_bearer_token,
    health_check_passes_with_a_templated_topic,
    collector_key_unlocks_the_authenticated_endpoint,
    collector_key_scopes_the_request,
    revoked_and_expired_keys_are_refused,
    unavailable_key_store_is_reported_and_refuses_requests,
];

async fn events_are_published_to_the_workspace_topic(ctx: &Ctx) -> anyhow::Result<()> {
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
            event["workspace_id"] == scope.workspace_id.as_str(),
            "wrong workspace in {event}"
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

async fn each_workspace_gets_its_own_topic(ctx: &Ctx) -> anyhow::Result<()> {
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

async fn events_for_another_workspace_are_rejected(ctx: &Ctx) -> anyhow::Result<()> {
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
    let mut without_workspace = event.clone();
    without_workspace
        .as_object_mut()
        .unwrap()
        .remove("workspace_id");

    for (label, headers, body) in [
        (
            "missing x-workspace-id",
            vec![("x-org-id", scope.org_id.as_str())],
            event.clone(),
        ),
        (
            "invalid x-org-id slug",
            vec![
                ("x-org-id", "Not_A_Slug"),
                ("x-workspace-id", scope.workspace_id.as_str()),
            ],
            event.clone(),
        ),
        (
            "x-workspace-id of another org",
            vec![
                ("x-org-id", "e2e_other_org"),
                ("x-workspace-id", scope.workspace_id.as_str()),
            ],
            event.clone(),
        ),
        (
            "x-workspace-id without the org",
            vec![
                ("x-org-id", scope.org_id.as_str()),
                ("x-workspace-id", scope.workspace_id.split_once('-').unwrap().1),
            ],
            event.clone(),
        ),
        (
            "event without workspace_id",
            scope.headers(),
            without_workspace,
        ),
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

/// A key for the scope publishes, marked authenticated, and the request
/// needn't name its scope: the key does. A wrong secret for the same kid,
/// or a malformed key, is refused.
async fn collector_key_unlocks_the_authenticated_endpoint(ctx: &Ctx) -> anyhow::Result<()> {
    let health = ctx.http.get("/v1/health", None, &[]).await?;
    expect_status(&health, 200)?;
    let health = health.json()?;
    ensure!(
        health["authenticated_endpoint"] == "ok",
        "unexpected health {health}"
    );

    let scope = ctx.scope().await?;
    let key = ctx
        .keys
        .create_key(&scope.org_id, &scope.workspace_id, None)
        .await?;
    let wrong_secret = format!("Bearer sk_{}_{}", key.kid, "0".repeat(64));
    for bad in [wrong_secret.as_str(), "Bearer sk_nope", "Bearer not-a-key"] {
        let (_, event) = scope.event("login");
        let response = ctx
            .post_authenticated(&[("authorization", bad)], &[event])
            .await?;
        expect_status(&response, 401).with_context(|| bad.to_owned())?;
    }

    let (event_id, event) = scope.event("login");
    let response = ctx
        .post_authenticated(
            &[
                ("authorization", &key.bearer),
                ("x-forwarded-for", CLIENT_IP),
            ],
            &[event],
        )
        .await?;
    expect_status(&response, 200)?;

    let records = ctx.kafka.read(&scope.topic, 1).await?;
    expect_ids(&records, &[&event_id])?;
    ensure!(
        records[0].value["authenticated"] == true,
        "event not marked authenticated: {}",
        records[0].value
    );
    Ok(())
}

/// A key is good for its own org and workspace only: headers or events
/// naming another are refused, and nothing reaches the other's topic.
async fn collector_key_scopes_the_request(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let other = ctx.scope_in(&scope.org_id).await?;
    let key = ctx
        .keys
        .create_key(&scope.org_id, &scope.workspace_id, None)
        .await?;

    let mut headers = other.headers();
    headers.push(("authorization", &key.bearer));
    let (_, event) = other.event("login");
    let response = ctx.post_authenticated(&headers, &[event]).await?;
    expect_status(&response, 400).context("headers for another workspace")?;

    let (_, event) = other.event("login");
    let response = ctx
        .post_authenticated(&[("authorization", &key.bearer)], &[event])
        .await?;
    expect_status(&response, 400).context("an event for another workspace")?;

    expect_ids(&ctx.kafka.read(&other.topic, 0).await?, &[])?;
    expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
}

/// An expired key is refused; a revoked one is, once the collector's cache
/// of it lapses.
async fn revoked_and_expired_keys_are_refused(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let expired = ctx
        .keys
        .create_key(
            &scope.org_id,
            &scope.workspace_id,
            Some(chrono::Utc::now() - chrono::Duration::minutes(1)),
        )
        .await?;
    let (_, event) = scope.event("login");
    let response = ctx
        .post_authenticated(&[("authorization", &expired.bearer)], &[event])
        .await?;
    expect_status(&response, 401).context("expired key")?;

    let key = ctx
        .keys
        .create_key(&scope.org_id, &scope.workspace_id, None)
        .await?;
    let (event_id, event) = scope.event("login");
    let response = ctx
        .post_authenticated(&[("authorization", &key.bearer)], &[event])
        .await?;
    expect_status(&response, 200).context("before revoking")?;

    ctx.keys.revoke(&key.kid).await?;
    tokio::time::sleep(std::time::Duration::from_millis(
        KEY_CACHE_SECS * 1000 + 500,
    ))
    .await;
    let (_, event) = scope.event("login");
    let response = ctx
        .post_authenticated(&[("authorization", &key.bearer)], &[event])
        .await?;
    expect_status(&response, 401).context("after revoking")?;

    expect_ids(&ctx.kafka.read(&scope.topic, 1).await?, &[&event_id])
}

/// With its key store unreachable, or not configured, the collector still
/// serves the open endpoints; health says why the authenticated one can't
/// work, and it answers 503 without publishing.
async fn unavailable_key_store_is_reported_and_refuses_requests(ctx: &Ctx) -> anyhow::Result<()> {
    for (store, reported) in [
        (
            Some("postgres://nobody:none@127.0.0.1:1/none"),
            "Collector keys unavailable",
        ),
        (None, "not configured"),
    ] {
        let collector = ctx.collector_with_keys_at(store).await?;
        let result = async {
            let http = Http::new(collector.base_url());

            let health = http.get("/v1/health", None, &[]).await?;
            expect_status(&health, 200)?;
            let health = health.json()?;
            let said = health["authenticated_endpoint"]
                .as_str()
                .unwrap_or_default();
            ensure!(
                health["status"] == "ok" && said.contains(reported),
                "health does not report {reported:?}: {health}"
            );

            let scope = ctx.scope().await?;
            let bearer = format!("Bearer sk_{}_{}", "a".repeat(16), "b".repeat(64));
            let (_, event) = scope.event("login");
            let response = http
                .post_raw(
                    "/v1/events/authenticated",
                    &[("authorization", bearer.as_str())],
                    ndjson(&[event]),
                )
                .await?;
            expect_status(&response, 503)?;
            expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
        }
        .await
        .with_context(|| format!("key store {store:?}"));
        if result.is_err() {
            collector.print_log_tail();
            return result;
        }
    }
    Ok(())
}

// ============ Helpers ============

fn event_for(org_id: &str, workspace_id: &str, name: &str) -> (String, Value) {
    let id = uuid::Uuid::new_v4().to_string();
    let event = json!({
        "envelop_version": "1.0",
        "id": id,
        "name": name,
        "org_id": org_id,
        "workspace_id": workspace_id,
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
