//! Event ingestion through a running events-collector into Kafka.
//!
//! The collector runs with `KAFKA_TOPIC` set to a per-run pattern
//! (`e2e-collector-<run>.{org_id}.{project_id}`), so every test scope
//! publishes to its own topic. Cases create those topics up front, send
//! events over HTTP and read what landed in Kafka.
//!
//! The authenticated endpoint takes per-project API keys from the
//! `collector_keys` table. The collector reads them from a throwaway
//! database on the compose Postgres, where cases insert their orgs,
//! projects and keys directly.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::{Context, ensure};
use canonical_event::CollectorKey;
use canonical_event::collector_key::{KID_BYTES, SECRET_BYTES};
use chrono::{DateTime, Utc};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::db::TestDb;
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
    pool: PgPool,
    database_url: String,
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
        self.http
            .post_raw("/v1/events", headers, ndjson(events))
            .await
    }

    /// POSTs `events` to the authenticated endpoint with `key` as the bearer
    /// token, and no org/project headers.
    async fn post_authenticated(
        &self,
        http: &Http,
        key: &str,
        events: &[Value],
    ) -> anyhow::Result<Response> {
        let bearer = format!("Bearer {key}");
        http.post_raw(
            "/v1/events/authenticated",
            &[("authorization", &bearer), ("x-forwarded-for", CLIENT_IP)],
            ndjson(events),
        )
        .await
    }

    /// Records `scope`'s org and project in the database, as the dashboard
    /// would have.
    async fn register(&self, scope: &Scope) -> anyhow::Result<()> {
        sqlx::query("INSERT INTO organizations (id, name) VALUES ($1, $1) ON CONFLICT DO NOTHING")
            .bind(&scope.org_id)
            .execute(&self.pool)
            .await?;
        sqlx::query(
            "INSERT INTO projects (id, organization_id, name, status) \
             VALUES ($1, $2, $1, 'ACTIVE') ON CONFLICT DO NOTHING",
        )
        .bind(&scope.project_id)
        .bind(&scope.org_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// A new API key for `scope`'s project, expiring at `expires_at`;
    /// returns it in full.
    async fn create_key(
        &self,
        scope: &Scope,
        expires_at: Option<DateTime<Utc>>,
    ) -> anyhow::Result<CollectorKey> {
        self.register(scope).await?;
        let key = CollectorKey::new(
            rand::random::<[u8; KID_BYTES]>(),
            rand::random::<[u8; SECRET_BYTES]>(),
        );
        sqlx::query(
            "INSERT INTO collector_keys (kid, project_id, hashed_secret, key_name, expires_at) \
             VALUES ($1, $2, $3, 'e2e', $4)",
        )
        .bind(key.kid())
        .bind(&scope.project_id)
        .bind(key.hashed_secret().as_slice())
        .bind(expires_at)
        .execute(&self.pool)
        .await?;
        Ok(key)
    }

    async fn revoke_key(&self, key: &CollectorKey) -> anyhow::Result<()> {
        sqlx::query("UPDATE collector_keys SET revoked_at = NOW() WHERE kid = $1")
            .bind(key.kid())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// A collector of its own with `extra` settings on top of the suite's.
    async fn collector_with(&self, extra: &[(&'static str, String)]) -> anyhow::Result<Service> {
        let mut env = collector_env(&self.kafka, &self.topic_prefix, &self.database_url);
        env.extend_from_slice(extra);
        Service::start(PACKAGE, "SERVER_ADDR", None, &env).await
    }
}

fn ndjson(events: &[Value]) -> String {
    events
        .iter()
        .map(Value::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// What every collector in this suite runs with. Key lookups are not
/// cached, so a revocation applies at once.
fn collector_env(
    kafka: &Kafka,
    topic_prefix: &str,
    database_url: &str,
) -> Vec<(&'static str, String)> {
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
            format!("{topic_prefix}.{{org_id}}.{{project_id}}"),
        ),
        ("DATABASE_URL", database_url.to_owned()),
        ("KEY_CACHE_TTL_SECS", "0".to_owned()),
        ("RUST_LOG", "warn".to_owned()),
    ]
}

/// Builds events-collector, creates a throwaway database, starts the
/// collector against Kafka and the database, runs the selected cases and
/// deletes the topics and database. Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;
    // Build first: a failed build exits the process, which must not leak a database.
    Service::build(PACKAGE);

    let kafka = Kafka::connect().await?;
    let db = TestDb::create("collector").await?;
    let topic_prefix = format!(
        "e2e-collector-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    let ctx = Ctx {
        http: Http::new(String::new()),
        kafka,
        pool: db.pool.clone(),
        database_url: db.url.clone(),
        topic_prefix,
    };
    let outcome = run_with_infra(ctx, &cases).await;
    db.drop_database().await?;
    outcome
}

async fn run_with_infra(mut ctx: Ctx, cases: &[&Case<Ctx>]) -> anyhow::Result<bool> {
    let server = Service::start(
        PACKAGE,
        "SERVER_ADDR",
        None,
        &collector_env(&ctx.kafka, &ctx.topic_prefix, &ctx.database_url),
    )
    .await;
    let passed = match server {
        Ok(server) => {
            ctx.http = Http::new(server.base_url());
            Ok(run_cases(&ctx, cases, |_| {}, || server.print_log_tail()).await)
        }
        Err(err) => Err(err),
    };
    let topics = ctx.kafka.delete_created().await;
    let passed = passed?;
    topics?;
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
    api_key_publishes_to_its_project,
    api_key_rejects_events_for_another_project,
    unusable_api_keys_are_refused,
    revoked_api_key_works_until_its_cached_lookup_expires,
    api_keys_without_a_database_are_reported_and_refused,
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

/// A key publishes to its own project's topic without org/project headers,
/// marked authenticated, and health reports the endpoint ok.
async fn api_key_publishes_to_its_project(ctx: &Ctx) -> anyhow::Result<()> {
    let health = ctx.http.get("/v1/health", None, &[]).await?;
    expect_status(&health, 200)?;
    let health = health.json()?;
    ensure!(
        health["authenticated_endpoint"] == "ok",
        "unexpected health {health}"
    );

    let scope = ctx.scope().await?;
    let key = ctx.create_key(&scope, None).await?;
    let (first_id, first) = scope.event("login");
    let (second_id, second) = scope.event("purchase");

    let response = ctx
        .post_authenticated(&ctx.http, &key.to_string(), &[first, second])
        .await?;
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
            event["authenticated"] == true,
            "event not marked authenticated: {event}"
        );
        ensure!(
            event["system_properties"]["ip_address"] == CLIENT_IP,
            "client IP not enriched in {event}"
        );
    }
    Ok(())
}

/// A key is for one project: events for another project of the same org are
/// rejected, and nothing is published.
async fn api_key_rejects_events_for_another_project(ctx: &Ctx) -> anyhow::Result<()> {
    let shop = ctx.scope().await?;
    let blog = ctx.scope_in(&shop.org_id).await?;
    let key = ctx.create_key(&shop, None).await?;
    ctx.register(&blog).await?;
    let (_, own) = shop.event("purchase");
    let (_, foreign) = blog.event("comment");

    let response = ctx
        .post_authenticated(&ctx.http, &key.to_string(), &[own, foreign])
        .await?;
    expect_status(&response, 400)?;
    ensure!(
        response.body.contains("event 1"),
        "error does not name the offending event: {}",
        response.body
    );

    expect_ids(&ctx.kafka.read(&shop.topic, 0).await?, &[]).context("shop topic")?;
    expect_ids(&ctx.kafka.read(&blog.topic, 0).await?, &[]).context("blog topic")
}

/// Malformed, unknown, wrong-secret, revoked and expired keys, and keys of
/// deleted projects or orgs, get 401 and publish nothing.
async fn unusable_api_keys_are_refused(ctx: &Ctx) -> anyhow::Result<()> {
    let scope = ctx.scope().await?;
    let valid = ctx.create_key(&scope, None).await?;
    let revoked = ctx.create_key(&scope, None).await?;
    ctx.revoke_key(&revoked).await?;
    let expired = ctx
        .create_key(&scope, Some(Utc::now() - chrono::Duration::minutes(1)))
        .await?;
    let unknown = CollectorKey::new(
        rand::random::<[u8; KID_BYTES]>(),
        rand::random::<[u8; SECRET_BYTES]>(),
    );
    // The valid key's kid with another secret.
    let wrong_secret = format!("sk_{}_{}", valid.kid(), "0".repeat(SECRET_BYTES * 2));

    let deleted_project = ctx.scope_in(&scope.org_id).await?;
    let deleted_project_key = ctx.create_key(&deleted_project, None).await?;
    sqlx::query("UPDATE projects SET deleted_at = NOW() WHERE id = $1")
        .bind(&deleted_project.project_id)
        .execute(&ctx.pool)
        .await?;
    let deleted_org = ctx.scope().await?;
    let deleted_org_key = ctx.create_key(&deleted_org, None).await?;
    sqlx::query("UPDATE organizations SET deleted_at = NOW() WHERE id = $1")
        .bind(&deleted_org.org_id)
        .execute(&ctx.pool)
        .await?;

    for (label, key, scope) in [
        ("malformed", "sk_not_a_key".to_owned(), &scope),
        ("unknown kid", unknown.to_string(), &scope),
        ("wrong secret", wrong_secret, &scope),
        ("revoked", revoked.to_string(), &scope),
        ("expired", expired.to_string(), &scope),
        (
            "deleted project",
            deleted_project_key.to_string(),
            &deleted_project,
        ),
        ("deleted org", deleted_org_key.to_string(), &deleted_org),
    ] {
        let (_, event) = scope.event("login");
        let response = ctx.post_authenticated(&ctx.http, &key, &[event]).await?;
        expect_status(&response, 401).context(label)?;
        ensure!(
            response
                .headers
                .get("www-authenticate")
                .is_some_and(|value| value == "Bearer"),
            "{label}: missing `WWW-Authenticate: Bearer` on {:?}",
            response.headers
        );
    }

    for scope in [&scope, &deleted_project, &deleted_org] {
        expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
            .with_context(|| format!("topic {}", scope.topic))?;
    }
    Ok(())
}

/// With caching on, a lookup is reused for `KEY_CACHE_TTL_SECS`: a revoked
/// key keeps working until then, and is refused after.
async fn revoked_api_key_works_until_its_cached_lookup_expires(ctx: &Ctx) -> anyhow::Result<()> {
    const TTL: Duration = Duration::from_secs(2);

    let collector = ctx
        .collector_with(&[("KEY_CACHE_TTL_SECS", TTL.as_secs().to_string())])
        .await?;
    let result = async {
        let http = Http::new(collector.base_url());
        let scope = ctx.scope().await?;
        let key = ctx.create_key(&scope, None).await?.to_string();

        let (cached_id, event) = scope.event("login");
        let response = ctx.post_authenticated(&http, &key, &[event]).await?;
        expect_status(&response, 200).context("before revocation")?;

        ctx.revoke_key(&CollectorKey::parse(&key).context("unparsable key")?)
            .await?;
        let (revoked_id, event) = scope.event("login");
        let response = ctx.post_authenticated(&http, &key, &[event]).await?;
        expect_status(&response, 200).context("revoked, lookup still cached")?;

        tokio::time::sleep(TTL + Duration::from_millis(500)).await;
        let (_, event) = scope.event("login");
        let response = ctx.post_authenticated(&http, &key, &[event]).await?;
        expect_status(&response, 401).context("revoked, cached lookup expired")?;

        expect_ids(
            &ctx.kafka.read(&scope.topic, 2).await?,
            &[&cached_id, &revoked_id],
        )
    }
    .await;
    if result.is_err() {
        collector.print_log_tail();
    }
    result
}

/// Without `DATABASE_URL`, health reports the configuration error, and the
/// authenticated endpoint refuses well-formed keys with 500 without
/// publishing.
async fn api_keys_without_a_database_are_reported_and_refused(ctx: &Ctx) -> anyhow::Result<()> {
    // Empty counts as unset, and overrides any `DATABASE_URL` inherited
    // from this process.
    let collector = ctx
        .collector_with(&[("DATABASE_URL", String::new())])
        .await?;
    let result = async {
        let http = Http::new(collector.base_url());

        let health = http.get("/v1/health", None, &[]).await?;
        expect_status(&health, 200)?;
        let health = health.json()?;
        let reported = health["authenticated_endpoint"]
            .as_str()
            .unwrap_or_default();
        ensure!(
            health["status"] == "ok" && reported.contains("DATABASE_URL"),
            "health does not report the missing DATABASE_URL: {health}"
        );

        let scope = ctx.scope().await?;
        let key = ctx.create_key(&scope, None).await?;
        let (_, event) = scope.event("login");
        let response = ctx
            .post_authenticated(&http, &key.to_string(), &[event])
            .await?;
        expect_status(&response, 500)?;
        expect_ids(&ctx.kafka.read(&scope.topic, 0).await?, &[])
    }
    .await;
    if result.is_err() {
        collector.print_log_tail();
    }
    result
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
