//! The Customers (CDP) integration, end to end: a project's events become
//! customer profiles, metrics and segments in the CDP, read back through
//! dashboard-server's `/api/cdp` routes.
//!
//! Runs dashboard-server and events-collector against the compose postgres
//! and redpanda, and a CDP running next to it — ingestor
//! consuming the per-project topics, the identity/metric/segment consumers,
//! and the control plane — configured as the CDP repo's
//! `deploy/compose/sudoquery.env` describes. The CDP is reached at
//! `CDP_CONTROL_PLANE_URL` with `CDP_PLATFORM_TOKEN` (defaults: that file's
//! local values).
//!
//! ClickHouse and OpenSearch are not involved: dashboard-server is pointed at
//! a stub answering 200 to everything, so provisioning and deletion run all
//! their steps but only the CDP workspace and the Kafka topic are real (the
//! `dashboard` suite covers the analytics steps).
//!
//! Projects use the real topic template, `events.{org_id}.{project_id}`,
//! since that is what the CDP consumes; every org and project gets fresh
//! random slugs, and everything a run provisions is removed afterwards
//! except the CDP tenants of its organizations (the CDP has no tenant
//! delete; they hold no data once their workspaces are gone).

use std::time::Duration;

use anyhow::{Context, ensure};
use serde_json::{Value, json};
use sqlx::PgPool;

use super::db::TestDb;
use super::kafka::Kafka;
use super::mock::{Mock, TestUser};
use super::server::Service;
use super::{Http, Response, run_cases, select};

const DASHBOARD: &str = "hyper-analytics-dashboard-server";
const COLLECTOR: &str = "event-collector";
const ADMIN_TOKEN: &str = "e2e-admin-token";
const PROJECT_PASSWORD: &str = "e2e-project-password";
const STUB_USER: &str = "e2e-admin";
const DEFAULT_CDP_URL: &str = "http://localhost:8082/cdp/control-plane";
const DEFAULT_CDP_TOKEN: &str = "local-sudoquery-platform-token";
const PROVISION_TIMEOUT: Duration = Duration::from_secs(60);
/// Events → profile: collector, CDP ingestor, resolver, metric fold.
const PROFILE_TIMEOUT: Duration = Duration::from_secs(60);
/// How long the CDP's metric fold (cdp-metric-recompute) may serve metric
/// definitions read before a change: it re-reads them every 60s, and folds
/// nothing for an event whose metrics it doesn't know yet.
const METRIC_DEFINITIONS_REFRESH: Duration = Duration::from_secs(65);
/// A new segment: definition refresh (≤60s) plus the backfill walk.
const SEGMENT_TIMEOUT: Duration = Duration::from_secs(150);
const POLL: Duration = Duration::from_millis(500);

pub(crate) struct Ctx {
    /// dashboard-server's `/api`.
    http: Http,
    /// events-collector.
    collector: Http,
    /// The CDP control plane, as its platform gateway.
    cdp: Cdp,
    pool: PgPool,
    mock: Mock,
    kafka: Kafka,
}

/// Direct control-plane access, for assertions on the CDP's own state.
struct Cdp {
    url: String,
    token: String,
    client: reqwest::Client,
}

impl Cdp {
    async fn get(&self, path: &str, tenant: &str) -> anyhow::Result<(u16, Value)> {
        let response = self
            .client
            .get(format!("{}{path}", self.url))
            .bearer_auth(&self.token)
            .header("X-CDP-Tenant", tenant)
            .send()
            .await
            .context("request to the CDP control plane failed")?;
        let status = response.status().as_u16();
        let body = response.json().await.unwrap_or(Value::Null);
        Ok((status, body))
    }

    async fn workspaces(&self, tenant: &str) -> anyhow::Result<Vec<String>> {
        let (status, body) = self
            .get(&format!("/v1/tenants/{tenant}/workspaces"), tenant)
            .await?;
        if status == 404 {
            return Ok(Vec::new());
        }
        ensure!(
            status == 200,
            "listing CDP workspaces: HTTP {status}: {body}"
        );
        Ok(body["workspaces"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|w| w["workspace_id"].as_str().map(str::to_owned))
            .collect())
    }

    async fn delete_workspace(&self, tenant: &str, workspace: &str) -> anyhow::Result<()> {
        let response = self
            .client
            .delete(format!(
                "{}/v1/tenants/{tenant}/workspaces/{workspace}?confirm={workspace}",
                self.url
            ))
            .bearer_auth(&self.token)
            .header("X-CDP-Tenant", tenant)
            .send()
            .await?;
        let status = response.status().as_u16();
        ensure!(
            status < 300 || status == 404,
            "deleting CDP workspace {tenant}/{workspace}: HTTP {status}"
        );
        Ok(())
    }
}

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;
    Service::build(DASHBOARD);
    Service::build(COLLECTOR);

    let cdp = Cdp {
        url: env_or("CDP_CONTROL_PLANE_URL", DEFAULT_CDP_URL),
        token: env_or("CDP_PLATFORM_TOKEN", DEFAULT_CDP_TOKEN),
        client: reqwest::Client::new(),
    };
    let health = cdp.client.get(format!("{}/healthz", cdp.url)).send().await;
    ensure!(
        health.is_ok_and(|r| r.status().is_success()),
        "the CDP control plane is not reachable at {} — start the CDP next to this \
         stack (CDP repo: deploy/compose/sudoquery.env) or set CDP_CONTROL_PLANE_URL",
        cdp.url
    );

    let kafka = Kafka::connect().await?;
    let db = TestDb::create().await?;
    let outcome = run_with_db(&db, cdp, kafka, &cases).await;
    db.drop_database().await?;
    outcome
}

async fn run_with_db(
    db: &TestDb,
    cdp: Cdp,
    kafka: Kafka,
    cases: &[&super::Case<Ctx>],
) -> anyhow::Result<bool> {
    let mock = Mock::start().await?;
    let analytics = analytics_stub().await?;
    let dashboard_env = vec![
        ("DATABASE_URL", db.url.clone()),
        ("KEYCLOAK_URL", mock.keycloak_url().to_owned()),
        ("KEYCLOAK_REALM", super::mock::REALM.to_owned()),
        ("KEYCLOAK_ADMIN_USER", "e2e-admin".to_owned()),
        ("KEYCLOAK_ADMIN_PASS", "e2e-admin".to_owned()),
        ("CLICKHOUSE_URL", analytics.clone()),
        ("CLICKHOUSE_ADMIN_URL", analytics.clone()),
        ("CLICKHOUSE_ADMIN_USER", STUB_USER.to_owned()),
        ("CLICKHOUSE_ADMIN_PASSWORD", STUB_USER.to_owned()),
        ("CLICKHOUSE_PROJECT_PASSWORD", PROJECT_PASSWORD.to_owned()),
        (
            "CLICKHOUSE_KAFKA_BROKERS",
            kafka.bootstrap_servers().to_owned(),
        ),
        ("KAFKA_BROKER_URL", kafka.bootstrap_servers().to_owned()),
        (
            "PROJECT_TOPIC_TEMPLATE",
            "events.{org_id}.{project_id}".to_owned(),
        ),
        ("OPENSEARCH_URL", analytics),
        ("ADMIN_API_TOKEN", ADMIN_TOKEN.to_owned()),
        ("CDP_CONTROL_PLANE_URL", cdp.url.clone()),
        ("CDP_PLATFORM_TOKEN", cdp.token.clone()),
        ("RUST_LOG", "warn".to_owned()),
    ];
    let server = Service::start(DASHBOARD, "DASHBOARD_BIND_ADDR", None, &dashboard_env).await?;
    let collector_env = vec![
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
        ("KAFKA_TOPIC", "events.{org_id}.{project_id}".to_owned()),
        ("RUST_LOG", "warn".to_owned()),
    ];
    let collector = Service::start(COLLECTOR, "SERVER_ADDR", None, &collector_env).await?;

    let ctx = Ctx {
        http: Http::new(format!("{}/api", server.base_url())),
        collector: Http::new(collector.base_url()),
        cdp,
        pool: db.pool.clone(),
        mock,
        kafka,
    };
    let passed = run_cases(
        &ctx,
        cases,
        |_| {},
        || {
            server.print_log_tail();
            collector.print_log_tail();
        },
    )
    .await;

    // Remove whatever the cases left provisioned; every step runs.
    cleanup(&ctx).await?;
    Ok(passed)
}

/// Stands in for ClickHouse and OpenSearch: 200 with an empty body to any
/// request. Returns its base URL.
async fn analytics_stub() -> anyhow::Result<String> {
    let app = axum::Router::new().fallback(|| async { "" });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, app).await });
    Ok(base_url)
}

/// Drops every project's ClickHouse database and user, topic and CDP
/// workspace — deleted projects already lost theirs; this is idempotent.
async fn cleanup(ctx: &Ctx) -> anyhow::Result<()> {
    let projects: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT id, organization_id FROM projects")
            .fetch_all(&ctx.pool)
            .await?;
    let mut topics = Vec::new();
    let mut first_error = None;
    for (project_id, org_id) in projects {
        let outcome = async {
            if let Some(org_id) = &org_id {
                ctx.cdp.delete_workspace(org_id, &project_id).await?;
                topics.extend(
                    ctx.kafka
                        .topics_with_prefix(&format!("events.{org_id}.{project_id}"))
                        .await?,
                );
            }
            anyhow::Ok(())
        }
        .await;
        if let Err(err) = outcome {
            first_error.get_or_insert(err);
        }
    }
    ctx.kafka.adopt(topics);
    let deleted = ctx.kafka.delete_created().await;
    match first_error {
        Some(err) => Err(err),
        None => deleted,
    }
}

cases![Ctx:
    provisioning_creates_the_cdp_workspace,
    events_become_profiles_metrics_and_segments,
    cdp_reads_need_project_membership,
    segment_writes_are_pinned_to_the_project,
    project_deletion_removes_the_cdp_workspace,
];

// ============ Cases ============

async fn provisioning_creates_the_cdp_workspace(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.mock.token(&TestUser::new("Cdp Admin"));
    let (org_id, project_id) = active_project(ctx, &token, "Cdp Org", "Cdp Project").await?;

    let workspaces = ctx.cdp.workspaces(&org_id).await?;
    ensure!(
        workspaces.contains(&project_id),
        "CDP tenant {org_id} has workspaces {workspaces:?}, not {project_id}"
    );
    // The tenant starts with the default identifier vocabulary and no events.
    let response = cdp_get(ctx, &token, &org_id, &project_id, "/cdp/event-types").await?;
    expect_status(&response, 200)?;
    ensure!(
        response.json()?["event_types"] == json!([]),
        "new tenant has event types: {}",
        response.body
    );
    Ok(())
}

/// The headline flow: declare the vocabulary, send events through the
/// collector, find the customer, and watch a segment fill.
async fn events_become_profiles_metrics_and_segments(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.mock.token(&TestUser::new("Shop Admin"));
    let (org_id, project_id) = active_project(ctx, &token, "Shop Org", "Shop Project").await?;
    declare_shop_vocabulary(ctx, &token, &org_id, &project_id).await?;
    // New metrics reach the CDP's fold within a minute; events before that
    // would be resolved but never folded into them.
    tokio::time::sleep(METRIC_DEFINITIONS_REFRESH).await;

    let suffix = &uuid::Uuid::new_v4().simple().to_string()[..8];
    let email = format!("buyer-{suffix}@example.com");
    let device = format!("device-{suffix}");

    // An anonymous visit through the collector's public route: the bare
    // anon_id becomes a cookie identifier.
    let visit = event(
        &org_id,
        &project_id,
        "page_viewed",
        &device,
        json!({ "path": "/" }),
    );
    // An undeclared event: the CDP skips it.
    let click = event(&org_id, &project_id, "button_clicked", &device, json!({}));
    let response = ctx
        .collector
        .post_raw(
            "/v1/events",
            &[("x-org-id", &org_id), ("x-project-id", &project_id)],
            format!("{visit}\n{click}"),
        )
        .await?;
    ensure!(
        (200..300).contains(&response.status),
        "collector refused events: {} {}",
        response.status,
        response.body
    );

    // Two orders from the logged-in customer, as the collector's
    // authenticated route publishes them (its KMS bearer path is covered by
    // the `collector` suite): the same device, plus the email. The first is
    // delivered twice — the CDP must count it once (dedup on order_id).
    let topic = format!("events.{org_id}.{project_id}");
    let mut orders = Vec::new();
    for (order_id, amount) in [("A-1", 120), ("A-2", 80)] {
        let mut order = event(
            &org_id,
            &project_id,
            "order_completed",
            &device,
            json!({
                "order_id": order_id,
                "amount": amount,
                "identifiers": { "email": email, "cookie": device },
            }),
        );
        order["authenticated"] = json!(true);
        orders.push(order.to_string());
    }
    for order in [&orders[0], &orders[0], &orders[1]] {
        ctx.kafka.produce(&topic, &device, order.as_bytes()).await?;
    }

    // The customer resolves by email, with both identifiers and the folded
    // metrics.
    let (tok, org, proj) = (token.as_str(), org_id.as_str(), project_id.as_str());
    let lookup = format!("/cdp/profiles/lookup?type=email&value={email}");
    let lookup = lookup.as_str();
    let profile = poll(PROFILE_TIMEOUT, "the customer's metrics", || async move {
        let response = cdp_get(ctx, tok, org, proj, lookup).await?;
        if response.status != 200 {
            return Ok(None);
        }
        let profile = response.json()?;
        let ready = metric(&profile, "order_count").as_deref() == Some("2");
        Ok(ready.then_some(profile))
    })
    .await?;
    ensure!(
        metric(&profile, "total_spend").as_deref() == Some("200"),
        "total_spend is not 120 + 80: {profile}"
    );
    let types: Vec<&str> = profile["identifiers"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|i| i["type"].as_str())
        .collect();
    ensure!(
        types.contains(&"email") && types.contains(&"cookie"),
        "profile identifiers are {types:?}: {profile}"
    );
    let profile_id = str_field(&profile, "profile_id")?.to_owned();

    // The anonymous visit landed on the same customer.
    let by_device = cdp_get(
        ctx,
        &token,
        &org_id,
        &project_id,
        &format!("/cdp/profiles/lookup?type=cookie&value={device}"),
    )
    .await?;
    expect_status(&by_device, 200)?;
    ensure!(
        by_device.json()?["profile_id"] == profile_id.as_str(),
        "the device resolves to another profile: {}",
        by_device.body
    );

    let summary = cdp_get(ctx, &token, &org_id, &project_id, "/cdp/summary").await?;
    expect_status(&summary, 200)?;
    ensure!(
        summary.json()?["workspace_customers"] == 1,
        "expected one customer: {}",
        summary.body
    );

    // A segment over the folded metric backfills the customer.
    let response = ctx
        .http
        .put(
            "/cdp/segments/big-spenders",
            Some(&token),
            &scope(&org_id, &project_id),
            json!({
                "display_name": "Big spenders",
                "predicate": { "metric": "total_spend", "op": "gte", "value": 150 },
            }),
        )
        .await?;
    ensure!(
        (200..300).contains(&response.status),
        "creating the segment: {} {}",
        response.status,
        response.body
    );
    let pid = profile_id.as_str();
    poll(SEGMENT_TIMEOUT, "the segment to backfill", || async move {
        let response = cdp_get(ctx, tok, org, proj, "/cdp/segments/big-spenders/members").await?;
        expect_status(&response, 200)?;
        let members = response.json()?;
        let found = members["members"]
            .as_array()
            .is_some_and(|m| m.iter().any(|m| m["profile_id"] == pid));
        Ok(found.then_some(()))
    })
    .await?;

    let segments = cdp_get(
        ctx,
        &token,
        &org_id,
        &project_id,
        &format!("/cdp/profiles/{profile_id}/segments"),
    )
    .await?;
    expect_status(&segments, 200)?;
    ensure!(
        segments.body.contains("big-spenders"),
        "profile segments miss big-spenders: {}",
        segments.body
    );
    Ok(())
}

async fn cdp_reads_need_project_membership(ctx: &Ctx) -> anyhow::Result<()> {
    let owner = ctx.mock.token(&TestUser::new("Owner"));
    let (org_id, project_id) =
        active_project(ctx, &owner, "Private Org", "Private Project").await?;
    let stranger = ctx.mock.token(&TestUser::new("Stranger"));

    for path in ["/cdp/summary", "/cdp/segments", "/cdp/event-types"] {
        let response = cdp_get(ctx, &stranger, &org_id, &project_id, path).await?;
        ensure!(
            response.status == 403,
            "a non-member read {path}: {} {}",
            response.status,
            response.body
        );
        let anonymous = ctx
            .http
            .get(path, None, &scope(&org_id, &project_id))
            .await?;
        expect_status(&anonymous, 401).with_context(|| format!("anonymous {path}"))?;
    }
    // Vocabulary writes are organization-admin only.
    let response = ctx
        .http
        .put(
            "/cdp/event-types/signup",
            Some(&stranger),
            &[("X-Organization-Id", &org_id)],
            json!({}),
        )
        .await?;
    ensure!(
        response.status == 403,
        "a stranger declared an event: {} {}",
        response.status,
        response.body
    );
    Ok(())
}

/// The caller can't point a segment (or a segment list) at another project.
async fn segment_writes_are_pinned_to_the_project(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.mock.token(&TestUser::new("Pinned Admin"));
    let (org_id, project_a) = active_project(ctx, &token, "Pinned Org", "Project A").await?;
    let project_b = create_pending_project(ctx, &token, &org_id, "Project B").await?;
    provision_and_wait(ctx, &project_b).await?;
    declare_shop_vocabulary(ctx, &token, &org_id, &project_a).await?;

    let response = ctx
        .http
        .put(
            "/cdp/segments/pinned",
            Some(&token),
            &scope(&org_id, &project_a),
            json!({
                "workspace_id": project_b,
                "display_name": "Pinned",
                "predicate": { "metric": "order_count", "op": "gte", "value": 1 },
            }),
        )
        .await?;
    ensure!(
        (200..300).contains(&response.status),
        "creating the segment: {} {}",
        response.status,
        response.body
    );

    let in_a = cdp_get(ctx, &token, &org_id, &project_a, "/cdp/segments/pinned").await?;
    expect_status(&in_a, 200)?;
    ensure!(
        in_a.json()?["workspace_id"] == project_a.as_str(),
        "segment landed elsewhere: {}",
        in_a.body
    );
    let list_b = cdp_get(
        ctx,
        &token,
        &org_id,
        &project_b,
        &format!("/cdp/segments?workspace_id={project_a}"),
    )
    .await?;
    expect_status(&list_b, 200)?;
    ensure!(
        !list_b.body.contains("\"pinned\""),
        "project B lists project A's segment: {}",
        list_b.body
    );
    Ok(())
}

async fn project_deletion_removes_the_cdp_workspace(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.mock.token(&TestUser::new("Leaving Admin"));
    let (org_id, project_id) =
        active_project(ctx, &token, "Leaving Org", "Leaving Project").await?;
    ensure!(ctx.cdp.workspaces(&org_id).await?.contains(&project_id));

    let response = ctx
        .http
        .delete("/project", Some(&token), &scope(&org_id, &project_id))
        .await?;
    expect_status(&response, 204)?;
    let workspaces = ctx.cdp.workspaces(&org_id).await?;
    ensure!(
        !workspaces.contains(&project_id),
        "deleted project's CDP workspace remains: {workspaces:?}"
    );
    Ok(())
}

// ============ Helpers ============

fn scope<'a>(org_id: &'a str, project_id: &'a str) -> [(&'static str, &'a str); 2] {
    [("X-Organization-Id", org_id), ("X-Project-Id", project_id)]
}

async fn cdp_get(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    project_id: &str,
    path: &str,
) -> anyhow::Result<Response> {
    ctx.http
        .get(path, Some(token), &scope(org_id, project_id))
        .await
}

/// An organization with one provisioned (ACTIVE) project.
async fn active_project(
    ctx: &Ctx,
    token: &str,
    org: &str,
    project: &str,
) -> anyhow::Result<(String, String)> {
    let response = ctx
        .http
        .post("/organizations", Some(token), &[], json!({ "name": org }))
        .await?;
    expect_status(&response, 201).with_context(|| format!("creating organization {org:?}"))?;
    let org_id = str_field(&response.json()?, "id")?.to_owned();
    let project_id = create_pending_project(ctx, token, &org_id, project).await?;
    provision_and_wait(ctx, &project_id).await?;
    Ok((org_id, project_id))
}

async fn create_pending_project(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    name: &str,
) -> anyhow::Result<String> {
    let response = ctx
        .http
        .post(
            "/projects",
            Some(token),
            &[("X-Organization-Id", org_id)],
            json!({ "name": name }),
        )
        .await?;
    expect_status(&response, 201).with_context(|| format!("creating project {name:?}"))?;
    Ok(str_field(&response.json()?, "id")?.to_owned())
}

async fn provision_and_wait(ctx: &Ctx, project_id: &str) -> anyhow::Result<()> {
    let admin = [("X-Admin-Token", ADMIN_TOKEN)];
    let response = ctx
        .http
        .post(
            &format!("/admin/projects/{project_id}/provision"),
            None,
            &admin,
            json!({}),
        )
        .await?;
    expect_status(&response, 202)?;
    let outcome = poll(PROVISION_TIMEOUT, "provisioning", || async move {
        let response = ctx
            .http
            .get(&format!("/admin/projects/{project_id}"), None, &admin)
            .await?;
        expect_status(&response, 200)?;
        let body = response.json()?;
        Ok((body["status"] != "CREATING").then_some(body))
    })
    .await?;
    ensure!(
        outcome["status"] == "ACTIVE",
        "provisioning {project_id} failed: {outcome}"
    );
    Ok(())
}

/// The events and metrics the shop cases use, declared through the gateway
/// as the organization admin.
async fn declare_shop_vocabulary(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    project_id: &str,
) -> anyhow::Result<()> {
    let headers = scope(org_id, project_id);
    for (name, body) in [
        ("page_viewed", json!({})),
        (
            "order_completed",
            json!({ "dedup_entity": "order", "dedup_key_paths": ["properties.order_id"] }),
        ),
    ] {
        let response = ctx
            .http
            .put(
                &format!("/cdp/event-types/{name}"),
                Some(token),
                &headers,
                body,
            )
            .await?;
        ensure!(
            (200..300).contains(&response.status),
            "declaring {name}: {} {}",
            response.status,
            response.body
        );
    }
    for metric in [
        json!({ "metric_name": "total_spend", "event_name": "order_completed",
                "value_kind": "property_number", "value_path": "properties.amount" }),
        json!({ "metric_name": "order_count", "event_name": "order_completed", "value_kind": "count" }),
        json!({ "metric_name": "last_seen_at", "event_name": "page_viewed", "value_kind": "occurred_at_epoch" }),
    ] {
        let response = ctx
            .http
            .put(
                "/cdp/metric-definitions",
                Some(token),
                &headers,
                metric.clone(),
            )
            .await?;
        ensure!(
            (200..300).contains(&response.status),
            "defining {metric}: {} {}",
            response.status,
            response.body
        );
    }
    Ok(())
}

/// A collector-shaped event (the shape events-collector publishes).
fn event(org_id: &str, project_id: &str, name: &str, anon_id: &str, properties: Value) -> Value {
    json!({
        "envelop_version": "1.0",
        "id": uuid::Uuid::new_v4().to_string(),
        "name": name,
        "org_id": org_id,
        "project_id": project_id,
        "anon_id": anon_id,
        "occured_at": chrono::Utc::now().to_rfc3339(),
        "properties": properties,
    })
}

/// A metric's value from a profile view, as its exact decimal string.
fn metric(profile: &Value, name: &str) -> Option<String> {
    profile["metrics"]
        .as_array()?
        .iter()
        .find(|m| m["metric"] == name)
        .and_then(|m| m["value"].as_str().map(str::to_owned))
}

/// Calls `check` every [`POLL`] until it returns a value, failing after
/// `timeout` with what it was waiting for.
async fn poll<T, F, Fut>(timeout: Duration, what: &str, mut check: F) -> anyhow::Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<Option<T>>>,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(value) = check().await? {
            return Ok(value);
        }
        ensure!(
            tokio::time::Instant::now() < deadline,
            "timed out after {timeout:?} waiting for {what}"
        );
        tokio::time::sleep(POLL).await;
    }
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

fn str_field<'a>(body: &'a Value, field: &str) -> anyhow::Result<&'a str> {
    body[field]
        .as_str()
        .with_context(|| format!("`{field}` missing from {body}"))
}
