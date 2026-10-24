//! Org and workspace creation against a running dashboard-server, backed by
//! the compose Postgres, Redpanda, ClickHouse and OpenSearch (only the
//! identity provider is a stand-in).
//!
//! A workspace is created `PENDING`; the admin API then provisions its Kafka
//! topic, ClickHouse database and user, and OpenSearch data stream. The
//! server runs with per-run topic and data stream templates, and every
//! workspace database, user, topic and data stream the run created is dropped
//! afterwards. A second server instance with a wrong ClickHouse admin
//! password produces real provisioning failures.

use std::time::Duration;

use anyhow::{Context, ensure};
use canonical_event::is_valid_slug;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::clickhouse::{ClickHouse, quote};
use super::db::TestDb;
use super::kafka::Kafka;
use super::mock::{Mock, TestUser};
use super::opensearch::OpenSearch;
use super::server::Service;
use super::{Http, Response, run_cases, select};

const PACKAGE: &str = "hyper-analytics-dashboard-server";

/// Length of the random suffix appended to every generated slug.
const SUFFIX_LEN: usize = 6;
/// Password the server gives every workspace's ClickHouse user.
const WORKSPACE_PASSWORD: &str = "e2e-workspace-password";
/// Every object `ckh/workspace.sql` creates in a workspace's database.
const WORKSPACE_OBJECTS: [&str; 7] = [
    "event_schema_catalog",
    "event_schema_catalog_from_events_mv",
    "events_kafka",
    "events_v2",
    "events_v2_mv",
    "user_events_v2",
    "user_events_v2_mv",
];
/// The tables a workspace's ClickHouse user may read.
const WORKSPACE_TABLES: [&str; 3] = ["event_schema_catalog", "events_v2", "user_events_v2"];
/// How long an event may take from the workspace topic to its tables.
const INGEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Token both servers require on the admin API.
const ADMIN_TOKEN: &str = "e2e-admin-token";
/// How long provisioning may take once the admin API started it.
const PROVISION_TIMEOUT: Duration = Duration::from_secs(60);
/// Older than the server's stale window (15 minutes), so an attempt this
/// old counts as dead.
const STALE: Duration = Duration::from_secs(60 * 60);

/// Everything a dashboard case can touch: the running servers, their
/// database, Kafka, ClickHouse and the dummy OIDC provider.
pub(crate) struct Ctx {
    http: Http,
    /// A server whose ClickHouse admin password is wrong, so provisioning
    /// fails after the topic was created.
    broken_http: Http,
    pool: PgPool,
    mock: Mock,
    kafka: Kafka,
    clickhouse: ClickHouse,
    opensearch: OpenSearch,
    /// Prefix of every workspace topic the servers create in this run.
    topic_prefix: String,
    /// Prefix of every workspace data stream the servers create in this run.
    data_stream_prefix: String,
}

impl Ctx {
    /// A fresh user with a unique OIDC subject.
    fn user(&self, name: &str) -> TestUser {
        TestUser::new(name)
    }

    /// A valid access token for `user`, signed by the published key.
    fn token(&self, user: &TestUser) -> String {
        self.mock.token(user)
    }

    async fn post(
        &self,
        path: &str,
        token: Option<&str>,
        headers: &[(&str, &str)],
        body: Value,
    ) -> anyhow::Result<Response> {
        self.http.post(path, token, headers, body).await
    }

    async fn get(
        &self,
        path: &str,
        token: Option<&str>,
        headers: &[(&str, &str)],
    ) -> anyhow::Result<Response> {
        self.http.get(path, token, headers).await
    }

    /// The topic the server creates for `org_id`/`workspace_id`.
    fn topic(&self, org_id: &str, workspace_id: &str) -> String {
        format!("{}.{org_id}.{workspace_id}", self.topic_prefix)
    }

    /// The data stream the server creates for `org_id`/`workspace_id`.
    fn data_stream(&self, org_id: &str, workspace_id: &str) -> String {
        format!("{}{org_id}-{workspace_id}", self.data_stream_prefix)
    }
}

/// Builds dashboard-server, creates a throwaway database, runs the selected
/// cases and drops the database and everything provisioned for its workspaces.
/// Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;

    // Build first: a failed build exits the process, which must not leak a database.
    Service::build(PACKAGE);
    let kafka = Kafka::connect().await?;
    let clickhouse = ClickHouse::connect().await?;
    let opensearch = OpenSearch::connect().await?;
    let db = TestDb::create().await?;
    let outcome = run_with_db(&db, kafka, clickhouse, opensearch, &cases).await;
    db.drop_database().await?;
    outcome
}

async fn run_with_db(
    db: &TestDb,
    kafka: Kafka,
    clickhouse: ClickHouse,
    opensearch: OpenSearch,
    cases: &[&super::Case<Ctx>],
) -> anyhow::Result<bool> {
    let topic_prefix = format!(
        "e2e-dashboard-{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    // Under `events-*`, so the index template makes them data streams.
    let data_stream_prefix = format!("events-{topic_prefix}-");
    let mock = Mock::start().await?;
    let env = |admin_password: &str| {
        vec![
            ("DATABASE_URL", db.url.clone()),
            ("KEYCLOAK_URL", mock.keycloak_url().to_owned()),
            ("KEYCLOAK_REALM", super::mock::REALM.to_owned()),
            ("KEYCLOAK_ADMIN_USER", "e2e-admin".to_owned()),
            ("KEYCLOAK_ADMIN_PASS", "e2e-admin".to_owned()),
            ("CLICKHOUSE_URL", clickhouse.url().to_owned()),
            ("CLICKHOUSE_ADMIN_URL", clickhouse.url().to_owned()),
            ("CLICKHOUSE_ADMIN_USER", clickhouse.user().to_owned()),
            ("CLICKHOUSE_ADMIN_PASSWORD", admin_password.to_owned()),
            (
                "CLICKHOUSE_WORKSPACE_PASSWORD",
                WORKSPACE_PASSWORD.to_owned(),
            ),
            (
                "CLICKHOUSE_KAFKA_BROKERS",
                clickhouse.kafka_brokers().to_owned(),
            ),
            ("KAFKA_BROKER_URL", kafka.bootstrap_servers().to_owned()),
            (
                "WORKSPACE_TOPIC_TEMPLATE",
                format!("{topic_prefix}.{{org_id}}.{{workspace_id}}"),
            ),
            ("OPENSEARCH_URL", opensearch.url().to_owned()),
            (
                "OPENSEARCH_INDEX",
                format!("{data_stream_prefix}{{org_id}}-{{workspace_id}}"),
            ),
            ("ADMIN_API_TOKEN", ADMIN_TOKEN.to_owned()),
            ("RUST_LOG", "warn".to_owned()),
        ]
    };
    let server = Service::start(
        PACKAGE,
        "DASHBOARD_BIND_ADDR",
        None,
        &env(clickhouse.password()),
    )
    .await?;
    let broken = Service::start(
        PACKAGE,
        "DASHBOARD_BIND_ADDR",
        None,
        &env("e2e-wrong-password"),
    )
    .await?;
    let ctx = Ctx {
        http: Http::new(format!("{}/api", server.base_url())),
        broken_http: Http::new(format!("{}/api", broken.base_url())),
        pool: db.pool.clone(),
        mock,
        kafka,
        clickhouse,
        opensearch,
        topic_prefix,
        data_stream_prefix,
    };

    let passed = run_cases(
        &ctx,
        cases,
        |_| {},
        || {
            server.print_log_tail();
            broken.print_log_tail();
        },
    )
    .await;
    // A failed case may leave an attempt running; let it end first, so
    // nothing it creates appears after cleanup looked.
    wait_for_attempts(&ctx).await;
    // Databases first: dropping them stops ClickHouse consuming the topics.
    // Every step runs even when an earlier one fails.
    let databases = drop_workspace_databases(&ctx).await;
    let topics = delete_topics(&ctx).await;
    let data_streams = ctx
        .opensearch
        .delete_data_streams(&ctx.data_stream_prefix)
        .await;
    databases.and(topics).and(data_streams)?;
    Ok(passed)
}

/// Waits, up to [`PROVISION_TIMEOUT`], until no workspace is `CREATING`.
/// Best effort: cleanup goes ahead either way.
async fn wait_for_attempts(ctx: &Ctx) {
    let deadline = tokio::time::Instant::now() + PROVISION_TIMEOUT;
    loop {
        let creating: Result<i64, _> =
            sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE status = 'CREATING'")
                .fetch_one(&ctx.pool)
                .await;
        match creating {
            Ok(0) => return,
            Ok(count) if tokio::time::Instant::now() >= deadline => {
                eprintln!("cleanup: {count} workspaces still CREATING; cleaning up anyway");
                return;
            }
            Ok(_) => tokio::time::sleep(Duration::from_millis(250)).await,
            Err(err) => {
                eprintln!("cleanup: cannot count CREATING workspaces: {err}");
                return;
            }
        }
    }
}

/// Deletes every topic the servers created in this run.
async fn delete_topics(ctx: &Ctx) -> anyhow::Result<()> {
    let topics = ctx.kafka.topics_with_prefix(&ctx.topic_prefix).await?;
    ctx.kafka.adopt(topics);
    ctx.kafka.delete_created().await
}

/// Drops the ClickHouse database and user of every workspace this run's
/// Postgres database knows, deleted or not.
async fn drop_workspace_databases(ctx: &Ctx) -> anyhow::Result<()> {
    let workspaces: Vec<String> = sqlx::query_scalar("SELECT id FROM workspaces")
        .fetch_all(&ctx.pool)
        .await?;
    for workspace_id in workspaces {
        ctx.clickhouse
            .query(&format!("DROP DATABASE IF EXISTS `{workspace_id}` SYNC"))
            .await?;
        ctx.clickhouse
            .query(&format!("DROP USER IF EXISTS {}", quote(&workspace_id)))
            .await?;
    }
    Ok(())
}

cases![Ctx:
    org_creation_returns_slug_and_admin_membership,
    org_ids_are_slugified_from_names,
    org_same_name_gets_distinct_ids,
    org_rejects_client_supplied_id,
    org_creation_requires_valid_token,
    workspace_creation_starts_pending,
    workspace_provisioning_creates_topic_database_user_and_data_stream,
    workspace_creation_requires_org_admin,
    workspace_creation_validates_org_header,
    workspace_provisioning_records_the_failed_step,
    workspace_provisioning_requires_admin_token,
    workspace_provisioning_runs_once_at_a_time,
    workspace_provisioning_staleness_ignores_workspace_edits,
    workspace_provisioning_superseded_attempt_records_nothing,
    workspace_provisioning_refuses_deleted_orgs,
    workspace_rejects_client_supplied_id,
    workspace_deletion_drops_topic_database_user_and_data_stream,
    workspace_deletion_and_provisioning_exclude_each_other,
    workspace_queries_target_the_workspace_database,
];

// ============ Organizations ============

async fn org_creation_returns_slug_and_admin_membership(ctx: &Ctx) -> anyhow::Result<()> {
    let user = ctx.user("Alice Admin");
    let token = ctx.token(&user);

    let response = ctx
        .post(
            "/organizations",
            Some(&token),
            &[],
            json!({ "name": "Acme Corp" }),
        )
        .await?;
    expect_status(&response, 201)?;
    let body = response.json()?;
    let org_id = str_field(&body, "id")?;
    expect_generated_slug(org_id, "acme-corp")?;
    ensure!(body["name"] == "Acme Corp", "unexpected name in {body}");

    let name: String = sqlx::query_scalar("SELECT name FROM organizations WHERE id = $1")
        .bind(org_id)
        .fetch_one(&ctx.pool)
        .await
        .context("organization row missing")?;
    ensure!(name == "Acme Corp", "stored name is {name:?}");

    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE keycloak_user_id = $1")
        .bind(&user.sub)
        .fetch_one(&ctx.pool)
        .await?;
    ensure!(
        users == 1,
        "expected the token's user to be created once, found {users}"
    );

    let role = org_role(ctx, &user.sub, org_id).await?;
    ensure!(
        role.as_deref() == Some("org_admin"),
        "creator role is {role:?}"
    );

    let mine = ctx.get("/my/organizations", Some(&token), &[]).await?;
    expect_status(&mine, 200)?;
    let mine = mine.json()?;
    let listed = mine
        .as_array()
        .context("/my/organizations is not an array")?
        .iter()
        .find(|org| org["id"] == org_id)
        .with_context(|| format!("{org_id} missing from /my/organizations: {mine}"))?;
    ensure!(
        listed["access_level"] == "org_admin",
        "unexpected listing {listed}"
    );
    Ok(())
}

async fn org_ids_are_slugified_from_names(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Slug Tester"));
    let long_name = "A very long organization name that keeps going on and on";
    for (name, base) in [
        ("Café Ñandú", "cafe-nandu"),
        ("  Hello,   World!  ", "hello-world"),
        ("42 Labs", "labs"),
        ("!!!", "org"),
        (long_name, "a-very-long-organizatio"),
    ] {
        let org_id = create_org(ctx, &token, name).await?;
        expect_generated_slug(&org_id, base).with_context(|| format!("name {name:?}"))?;
    }
    Ok(())
}

async fn org_same_name_gets_distinct_ids(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Twin Maker"));
    let first = create_org(ctx, &token, "Twin Org").await?;
    let second = create_org(ctx, &token, "Twin Org").await?;
    ensure!(first != second, "both organizations got id {first}");
    Ok(())
}

async fn org_rejects_client_supplied_id(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Id Picker"));
    let response = ctx
        .post(
            "/organizations",
            Some(&token),
            &[],
            json!({ "name": "Picked Id Org", "id": "my-chosen-org" }),
        )
        .await?;
    expect_client_error(&response)?;

    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM organizations WHERE id = 'my-chosen-org' OR name = 'Picked Id Org'",
    )
    .fetch_one(&ctx.pool)
    .await?;
    ensure!(
        rows == 0,
        "rejected request still inserted {rows} organization(s)"
    );
    Ok(())
}

async fn org_creation_requires_valid_token(ctx: &Ctx) -> anyhow::Result<()> {
    let user = ctx.user("Mallory");
    let body = json!({ "name": "Unauthorized Org" });
    let wrong_issuer = ctx
        .mock
        .token_with_issuer(&user, "http://127.0.0.1:1/realms/e2e");

    for (label, token) in [
        ("no token", None),
        ("unpublished signing key", Some(ctx.mock.rogue_token(&user))),
        ("wrong issuer", Some(wrong_issuer)),
        ("garbage", Some("not-a-jwt".to_owned())),
    ] {
        let response = ctx
            .post("/organizations", token.as_deref(), &[], body.clone())
            .await?;
        expect_status(&response, 401).with_context(|| label)?;
    }

    let created: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM organizations WHERE name = 'Unauthorized Org')
              + (SELECT count(*) FROM users WHERE keycloak_user_id = $1)",
    )
    .bind(&user.sub)
    .fetch_one(&ctx.pool)
    .await?;
    ensure!(
        created == 0,
        "unauthenticated requests created {created} row(s)"
    );
    Ok(())
}

// ============ Workspaces ============

async fn workspace_creation_starts_pending(ctx: &Ctx) -> anyhow::Result<()> {
    let user = ctx.user("Paula Workspace");
    let token = ctx.token(&user);
    let org_id = create_org(ctx, &token, "Workspace Home").await?;

    let response = ctx
        .post(
            "/workspaces",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Web Shop" }),
        )
        .await?;
    expect_status(&response, 201)?;
    let body = response.json()?;
    let workspace_id = str_field(&body, "id")?;
    expect_generated_slug(workspace_id, "web-shop")?;
    ensure!(
        body["organization_id"] == org_id.as_str() && body["status"] == "PENDING",
        "unexpected workspace {body}"
    );
    let workspace_token: uuid::Uuid = str_field(&body, "workspace_token")?
        .parse()
        .context("workspace_token is not a UUID")?;

    let (stored_org, status, deleted): (Option<String>, String, bool) = sqlx::query_as(
        "SELECT organization_id, status::text, deleted_at IS NOT NULL FROM workspaces WHERE id = $1",
    )
    .bind(workspace_id)
    .fetch_one(&ctx.pool)
    .await
    .context("workspace row missing")?;
    ensure!(
        stored_org.as_deref() == Some(org_id.as_str()),
        "stored org {stored_org:?}"
    );
    ensure!(status == "PENDING", "stored status {status}");
    ensure!(!deleted, "workspace is soft-deleted");

    let tokens: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM workspace_tokens WHERE workspace_id = $1 AND token = $2",
    )
    .bind(workspace_id)
    .bind(workspace_token)
    .fetch_one(&ctx.pool)
    .await?;
    ensure!(
        tokens == 1,
        "expected the returned token to be stored, found {tokens}"
    );

    let role = workspace_role(ctx, &user.sub, workspace_id).await?;
    ensure!(
        role.as_deref() == Some("workspace_admin"),
        "creator role is {role:?}"
    );

    // Nothing is provisioned until the admin API is called.
    let topics = ctx
        .kafka
        .topics_with_prefix(&ctx.topic(&org_id, workspace_id))
        .await?;
    ensure!(topics.is_empty(), "pending workspace has topics {topics:?}");
    expect_no_workspace_database(ctx, workspace_id).await?;
    let stream = ctx.data_stream(&org_id, workspace_id);
    ensure!(
        !ctx.opensearch.data_stream_exists(&stream).await?,
        "pending workspace has data stream {stream}"
    );

    let listed = ctx
        .get(
            "/workspaces",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
        )
        .await?;
    expect_status(&listed, 200)?;
    let listed = listed.json()?;
    ensure!(
        listed.as_array().is_some_and(|workspaces| workspaces
            .iter()
            .any(|p| p["id"] == workspace_id && p["status"] == "PENDING")),
        "{workspace_id} missing from GET /workspaces, or not PENDING: {listed}"
    );
    Ok(())
}

async fn workspace_provisioning_creates_topic_database_user_and_data_stream(
    ctx: &Ctx,
) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Paula Provisioner"));
    let org_id = create_org(ctx, &token, "Provisioned Home").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Web Shop").await?;

    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202)?;
    let body = response.json()?;
    ensure!(
        body == json!({ "id": workspace_id, "status": "CREATING" }),
        "unexpected provisioning answer {body}"
    );
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(
        outcome["status"] == "ACTIVE" && outcome["provisioning_error"].is_null(),
        "provisioning did not succeed: {outcome}"
    );

    let topic = ctx.topic(&org_id, &workspace_id);
    let topics = ctx.kafka.topics_with_prefix(&topic).await?;
    ensure!(
        topics.contains(&topic),
        "workspace topic {topic} was not created (found {topics:?})"
    );
    expect_workspace_database(ctx, &workspace_id, &topic).await?;
    expect_events_reach_workspace(ctx, &token, &org_id, &workspace_id, &topic).await?;
    expect_workspace_user_isolated(ctx, &workspace_id).await?;
    let stream = ctx.data_stream(&org_id, &workspace_id);
    ensure!(
        ctx.opensearch.data_stream_exists(&stream).await?,
        "data stream {stream} was not created"
    );

    let workspace = workspace_get(
        ctx,
        &token,
        &[
            ("X-Organization-Id", &org_id),
            ("X-Workspace-Id", &workspace_id),
        ],
        "/workspace".to_owned(),
    )
    .await?;
    ensure!(
        workspace["status"] == "ACTIVE",
        "GET /workspace after provisioning: {workspace}"
    );
    Ok(())
}

async fn workspace_creation_requires_org_admin(ctx: &Ctx) -> anyhow::Result<()> {
    let owner = ctx.token(&ctx.user("Org Owner"));
    let org_id = create_org(ctx, &owner, "Guarded Org").await?;
    let outsider = ctx.token(&ctx.user("Outsider"));

    let response = ctx
        .post(
            "/workspaces",
            Some(&outsider),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Sneaky Workspace" }),
        )
        .await?;
    expect_status(&response, 403)?;
    expect_no_workspace(ctx, "Sneaky Workspace").await
}

async fn workspace_creation_validates_org_header(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Header Tester"));
    let body = json!({ "name": "Headerless Workspace" });

    for (label, headers, status) in [
        ("missing header", vec![], 400),
        (
            "UUID org id",
            vec![("X-Organization-Id", "0b6bd7e7-1a4b-4d12-8fd3-9f8f0f2a1b2c")],
            400,
        ),
        (
            "uppercase org id",
            vec![("X-Organization-Id", "Acme-Corp-x1y2z3")],
            400,
        ),
        (
            "unknown org",
            vec![("X-Organization-Id", "no-such-org-zzzzzz")],
            404,
        ),
    ] {
        let response = ctx
            .post("/workspaces", Some(&token), &headers, body.clone())
            .await?;
        expect_status(&response, status).with_context(|| label)?;
    }
    expect_no_workspace(ctx, "Headerless Workspace").await
}

async fn workspace_provisioning_records_the_failed_step(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Unlucky"));
    let org_id = create_org(ctx, &token, "Flaky Warehouse").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Doomed Workspace").await?;

    // The broken server creates the topic, then ClickHouse rejects its
    // admin password.
    let response = provision(&ctx.broken_http, &workspace_id).await?;
    expect_status(&response, 202)?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(
        outcome["status"] == "ERRORED",
        "expected ERRORED: {outcome}"
    );
    let error = &outcome["provisioning_error"];
    ensure!(
        error["failed_step"] == "clickhouse_database"
            && error["completed_steps"] == json!(["kafka_topic"])
            && error["skipped_steps"] == json!(["clickhouse_user", "opensearch_index"])
            && error["error"].as_str().is_some_and(|e| !e.is_empty())
            && error["failed_at"].is_string(),
        "unexpected provisioning_error {error}"
    );

    // The workspace and its topic stay for the retry.
    let deleted: bool =
        sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM workspaces WHERE id = $1")
            .bind(&workspace_id)
            .fetch_one(&ctx.pool)
            .await?;
    ensure!(
        !deleted,
        "errored workspace {workspace_id} was soft-deleted"
    );
    let topic = ctx.topic(&org_id, &workspace_id);
    let topics = ctx.kafka.topics_with_prefix(&topic).await?;
    ensure!(
        topics.contains(&topic),
        "errored workspace lost its topic (found {topics:?})"
    );

    // A retry through the working server finishes the remaining steps.
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202)?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(
        outcome["status"] == "ACTIVE" && outcome["provisioning_error"].is_null(),
        "retry did not succeed: {outcome}"
    );
    expect_workspace_database(ctx, &workspace_id, &topic).await?;
    let stream = ctx.data_stream(&org_id, &workspace_id);
    ensure!(
        ctx.opensearch.data_stream_exists(&stream).await?,
        "data stream {stream} was not created by the retry"
    );
    Ok(())
}

async fn workspace_provisioning_requires_admin_token(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Impatient User"));
    let org_id = create_org(ctx, &token, "Impatient Org").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Waiting Workspace").await?;
    let path = format!("/admin/workspaces/{workspace_id}/provision");

    for (label, headers) in [
        ("no admin token", vec![]),
        (
            "wrong admin token",
            vec![("X-Admin-Token", "not-the-token")],
        ),
    ] {
        // A user's own bearer token is not enough either.
        let response = ctx.post(&path, Some(&token), &headers, json!({})).await?;
        expect_status(&response, 401).with_context(|| label)?;
        let response = ctx
            .get(&format!("/admin/workspaces/{workspace_id}"), None, &headers)
            .await?;
        expect_status(&response, 401).with_context(|| label)?;
    }

    let status = workspace_status(ctx, &workspace_id).await?;
    ensure!(status == "PENDING", "refused requests moved it to {status}");
    Ok(())
}

/// Only PENDING and ERRORED workspaces (or a CREATING one whose attempt went
/// stale) can be provisioned, and a CREATING workspace cannot be deleted.
async fn workspace_provisioning_runs_once_at_a_time(ctx: &Ctx) -> anyhow::Result<()> {
    for (label, workspace_id, status) in [
        ("unknown workspace", "no-such-workspace-zzzzzz", 404),
        ("invalid workspace id", "Not_A_Slug", 400),
    ] {
        let response = provision(&ctx.http, workspace_id).await?;
        expect_status(&response, status).with_context(|| label)?;
    }

    let token = ctx.token(&ctx.user("Busy Admin"));
    let org_id = create_org(ctx, &token, "Busy Org").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Busy Workspace").await?;

    // An attempt in progress.
    start_attempt(ctx, &workspace_id, Duration::ZERO).await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 409)?;
    ensure!(
        response.json()?["status"] == "CREATING",
        "unexpected refusal {}",
        response.body
    );
    let response = delete_workspace(ctx, &token, &org_id, &workspace_id).await?;
    expect_status(&response, 409).context("deleting a CREATING workspace")?;

    // An attempt that died with its server long ago is taken over.
    start_attempt(ctx, &workspace_id, STALE).await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202).context("taking over a stale attempt")?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(outcome["status"] == "ACTIVE", "takeover failed: {outcome}");

    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 409).context("provisioning an ACTIVE workspace")?;
    ensure!(
        response.json()?["status"] == "ACTIVE",
        "unexpected refusal {}",
        response.body
    );
    Ok(())
}

/// A workspace edit moves `updated_at`, but staleness counts from when the
/// attempt started.
async fn workspace_provisioning_staleness_ignores_workspace_edits(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Edit Admin"));
    let org_id = create_org(ctx, &token, "Edit Org").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Edited Workspace").await?;
    let headers = [
        ("X-Organization-Id", org_id.as_str()),
        ("X-Workspace-Id", workspace_id.as_str()),
    ];

    // A fresh attempt on a workspace untouched for an hour is not stale.
    start_attempt(ctx, &workspace_id, Duration::ZERO).await?;
    sqlx::query("UPDATE workspaces SET updated_at = NOW() - INTERVAL '1 hour' WHERE id = $1")
        .bind(&workspace_id)
        .execute(&ctx.pool)
        .await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 409).context("a fresh attempt on an old row")?;

    // A dead attempt stays stale when the workspace is renamed.
    start_attempt(ctx, &workspace_id, STALE).await?;
    let response = ctx
        .http
        .patch(
            "/workspace",
            Some(&token),
            &headers,
            json!({ "name": "Renamed Workspace" }),
        )
        .await?;
    expect_status(&response, 200).context("renaming the workspace")?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202).context("taking over after a rename")?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(outcome["status"] == "ACTIVE", "takeover failed: {outcome}");
    Ok(())
}

/// An attempt that was taken over cannot record how it ended.
async fn workspace_provisioning_superseded_attempt_records_nothing(
    ctx: &Ctx,
) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Superseded Admin"));
    let org_id = create_org(ctx, &token, "Superseded Org").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Raced Workspace").await?;

    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202)?;
    // Another attempt takes over while the first one is still running.
    let newer = uuid::Uuid::new_v4();
    let taken_over = sqlx::query(
        "UPDATE workspaces SET provisioning_attempt = $2 WHERE id = $1 AND status = 'CREATING'",
    )
    .bind(&workspace_id)
    .bind(newer)
    .execute(&ctx.pool)
    .await?
    .rows_affected();
    ensure!(
        taken_over == 1,
        "the first attempt ended before it could be superseded"
    );

    // The data stream is the first attempt's last step; give it a moment
    // to record its outcome after that.
    let stream = ctx.data_stream(&org_id, &workspace_id);
    let deadline = tokio::time::Instant::now() + PROVISION_TIMEOUT;
    while !ctx.opensearch.data_stream_exists(&stream).await? {
        ensure!(
            tokio::time::Instant::now() < deadline,
            "the first attempt never created {stream}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    tokio::time::sleep(Duration::from_secs(2)).await;

    let (status, attempt): (String, Option<uuid::Uuid>) =
        sqlx::query_as("SELECT status::text, provisioning_attempt FROM workspaces WHERE id = $1")
            .bind(&workspace_id)
            .fetch_one(&ctx.pool)
            .await?;
    ensure!(
        status == "CREATING" && attempt == Some(newer),
        "the superseded attempt recorded its outcome: {status}, attempt {attempt:?}"
    );

    // Once the newer attempt goes stale, the workspace can be provisioned.
    start_attempt(ctx, &workspace_id, STALE).await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202).context("taking over the stale attempt")?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(outcome["status"] == "ACTIVE", "takeover failed: {outcome}");
    Ok(())
}

async fn workspace_provisioning_refuses_deleted_orgs(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Gone Admin"));
    let org_id = create_org(ctx, &token, "Gone Org").await?;
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Orphan Workspace").await?;

    let response = ctx
        .http
        .delete(
            "/organization",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
        )
        .await?;
    expect_status(&response, 204).context("deleting the organization")?;

    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 409)?;
    ensure!(
        workspace_status(ctx, &workspace_id).await? == "PENDING",
        "a workspace of a deleted org left PENDING"
    );
    let topics = ctx
        .kafka
        .topics_with_prefix(&ctx.topic(&org_id, &workspace_id))
        .await?;
    ensure!(
        topics.is_empty(),
        "workspace of a deleted org got topics {topics:?}"
    );
    Ok(())
}

async fn workspace_deletion_drops_topic_database_user_and_data_stream(
    ctx: &Ctx,
) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Tidy Admin"));
    let org_id = create_org(ctx, &token, "Tidy Org").await?;
    let workspace_id = create_workspace(ctx, &token, &org_id, "Short Lived").await?;
    let topic = ctx.topic(&org_id, &workspace_id);
    expect_workspace_database(ctx, &workspace_id, &topic)
        .await
        .context("before deletion")?;
    let stream = ctx.data_stream(&org_id, &workspace_id);
    ensure!(
        ctx.opensearch.data_stream_exists(&stream).await?,
        "data stream {stream} missing before deletion"
    );

    let response = delete_workspace(ctx, &token, &org_id, &workspace_id).await?;
    expect_status(&response, 204)?;

    expect_no_workspace_database(ctx, &workspace_id).await?;
    let topics = ctx.kafka.topics_with_prefix(&topic).await?;
    ensure!(
        topics.is_empty(),
        "deleted workspace left topics {topics:?}"
    );
    ensure!(
        !ctx.opensearch.data_stream_exists(&stream).await?,
        "deleted workspace left data stream {stream}"
    );

    let deleted: bool =
        sqlx::query_scalar("SELECT deleted_at IS NOT NULL FROM workspaces WHERE id = $1")
            .bind(&workspace_id)
            .fetch_one(&ctx.pool)
            .await?;
    ensure!(deleted, "workspace {workspace_id} was not soft-deleted");
    Ok(())
}

/// Deletion waits for a live attempt but not a dead one, and provisioning
/// never takes a workspace that is being deleted.
async fn workspace_deletion_and_provisioning_exclude_each_other(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Exclusive Admin"));
    let org_id = create_org(ctx, &token, "Exclusive Org").await?;

    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Dying Attempt").await?;
    start_attempt(ctx, &workspace_id, Duration::ZERO).await?;
    let response = delete_workspace(ctx, &token, &org_id, &workspace_id).await?;
    expect_status(&response, 409).context("deleting during a live attempt")?;
    start_attempt(ctx, &workspace_id, STALE).await?;
    let response = delete_workspace(ctx, &token, &org_id, &workspace_id).await?;
    expect_status(&response, 204).context("deleting after the attempt died")?;

    // A deletion that failed half-way leaves the workspace DELETING.
    let workspace_id = create_pending_workspace(ctx, &token, &org_id, "Half Deleted").await?;
    sqlx::query("UPDATE workspaces SET status = 'DELETING' WHERE id = $1")
        .bind(&workspace_id)
        .execute(&ctx.pool)
        .await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 409).context("provisioning a DELETING workspace")?;
    ensure!(
        response.json()?["status"] == "DELETING",
        "unexpected refusal {}",
        response.body
    );
    let response = delete_workspace(ctx, &token, &org_id, &workspace_id).await?;
    expect_status(&response, 204).context("retrying the deletion")?;
    Ok(())
}

/// Two workspaces in one org, each with its own event: every query path of the
/// server reads the requesting workspace's database and nothing else.
async fn workspace_queries_target_the_workspace_database(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Query Admin"));
    let org_id = create_org(ctx, &token, "Query Org").await?;
    let shop = create_workspace(ctx, &token, &org_id, "Shop").await?;
    let blog = create_workspace(ctx, &token, &org_id, "Blog").await?;

    let shop_event = produce_event(
        ctx,
        &ctx.topic(&org_id, &shop),
        &org_id,
        &shop,
        "purchase",
        json!({ "amount": 30 }),
    )
    .await?;
    let blog_event = produce_event(
        ctx,
        &ctx.topic(&org_id, &blog),
        &org_id,
        &blog,
        "comment",
        json!({ "text": "hello" }),
    )
    .await?;
    wait_for_events(ctx, &token, &org_id, &shop, &[&shop_event]).await?;
    wait_for_events(ctx, &token, &org_id, &blog, &[&blog_event]).await?;

    for (workspace, event, property, other_workspace, other_event) in [
        (&shop, "purchase", "amount", &blog, "comment"),
        (&blog, "comment", "text", &shop, "purchase"),
    ] {
        let headers = [
            ("X-Organization-Id", org_id.as_str()),
            ("X-Workspace-Id", workspace.as_str()),
        ];
        let get = |path: String| workspace_get(ctx, &token, &headers, path);

        let database = workspace_query(
            ctx,
            &token,
            &org_id,
            workspace,
            "SELECT currentDatabase() AS db",
        )
        .await?;
        ensure!(
            database == json!([{ "db": workspace }]),
            "{workspace} queries run in {database}"
        );
        let events = get("/events".to_owned()).await?;
        ensure!(events == json!([event]), "{workspace} /events: {events}");
        let today = get("/events/today/count".to_owned()).await?;
        ensure!(
            today == json!({ "count": 1 }),
            "{workspace} /events/today/count: {today}"
        );
        let props = get(format!("/event_props?event_name={event}")).await?;
        ensure!(
            props == json!([property]),
            "{workspace} /event_props: {props}"
        );
        let other_props = get(format!("/event_props?event_name={other_event}")).await?;
        ensure!(
            other_props == json!([]),
            "{workspace} /event_props sees {other_workspace}'s {other_event}: {other_props}"
        );

        // Naming the other workspace's database explicitly is refused.
        let response = ctx
            .post(
                "/query",
                Some(&token),
                &headers,
                json!({ "query": format!("SELECT count() FROM `{other_workspace}`.events_v2") }),
            )
            .await?;
        expect_status(&response, 200)?;
        let body = response.json()?;
        ensure!(
            body["success"] == false
                && body["error"]
                    .as_str()
                    .is_some_and(|error| error.contains("ACCESS_DENIED")),
            "{workspace} could read {other_workspace}'s database: {body}"
        );
    }

    // A user outside the org cannot query the workspace at all.
    let outsider = ctx.token(&ctx.user("Query Outsider"));
    let response = ctx
        .post(
            "/query",
            Some(&outsider),
            &[("X-Organization-Id", &org_id), ("X-Workspace-Id", &shop)],
            json!({ "query": "SELECT count() FROM events_v2" }),
        )
        .await?;
    expect_status(&response, 403)
}

/// GETs `path` for the workspace in `headers`, expecting 200 and JSON.
async fn workspace_get(
    ctx: &Ctx,
    token: &str,
    headers: &[(&str, &str)],
    path: String,
) -> anyhow::Result<Value> {
    let response = ctx.get(&path, Some(token), headers).await?;
    expect_status(&response, 200).with_context(|| path.clone())?;
    response.json()
}

async fn workspace_rejects_client_supplied_id(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Workspace Id Picker"));
    let org_id = create_org(ctx, &token, "Id Picker Org").await?;

    let response = ctx
        .post(
            "/workspaces",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Picked Id Workspace", "id": "my-chosen-workspace" }),
        )
        .await?;
    expect_client_error(&response)?;
    expect_no_workspace(ctx, "Picked Id Workspace").await
}

// ============ Helpers ============

/// The workspace's database holds every object of `ckh/workspace.sql`, its
/// Kafka table reads the workspace topic, and its ClickHouse user may SELECT
/// exactly the workspace's tables.
async fn expect_workspace_database(
    ctx: &Ctx,
    workspace_id: &str,
    topic: &str,
) -> anyhow::Result<()> {
    let objects = ctx
        .clickhouse
        .rows(&format!(
            "SELECT name FROM system.tables WHERE database = {} ORDER BY name",
            quote(workspace_id)
        ))
        .await?;
    ensure!(
        objects == WORKSPACE_OBJECTS,
        "unexpected objects in database {workspace_id}: {objects:?}"
    );

    let kafka = ctx
        .clickhouse
        .query(&format!(
            "SELECT engine_full FROM system.tables \
             WHERE database = {} AND name = 'events_kafka' FORMAT TSVRaw",
            quote(workspace_id)
        ))
        .await?;
    ensure!(
        kafka.contains(&format!("kafka_topic_list = '{topic}'")),
        "events_kafka does not read {topic}: {kafka}"
    );

    let grants = ctx
        .clickhouse
        .rows(&format!(
            "SELECT access_type, database, table FROM system.grants \
             WHERE user_name = {} ORDER BY table",
            quote(workspace_id)
        ))
        .await?;
    let expected: Vec<String> = WORKSPACE_TABLES
        .iter()
        .map(|table| format!("SELECT\t{workspace_id}\t{table}"))
        .collect();
    ensure!(
        grants == expected,
        "unexpected grants for {workspace_id}: {grants:?}"
    );
    Ok(())
}

/// An event produced to the workspace topic reaches the workspace's tables and
/// catalog, read back through the server's own query endpoint (which runs
/// as the workspace user against the workspace database).
async fn expect_events_reach_workspace(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    workspace_id: &str,
    topic: &str,
) -> anyhow::Result<()> {
    let event_id = produce_event(
        ctx,
        topic,
        org_id,
        workspace_id,
        "checkout",
        json!({ "amount": 12.5 }),
    )
    .await?;
    wait_for_events(ctx, token, org_id, workspace_id, &[&event_id]).await?;

    let query = |sql| workspace_query(ctx, token, org_id, workspace_id, sql);
    let users = query("SELECT count() AS n FROM user_events_v2").await?;
    ensure!(
        users[0]["n"].to_string().trim_matches('"') == "1",
        "event missing from user_events_v2: {users}"
    );
    let catalog = query("SELECT property, type FROM event_schema_catalog").await?;
    ensure!(
        catalog == json!([{ "property": "amount", "type": "Float64" }]),
        "unexpected catalog: {catalog}"
    );
    Ok(())
}

/// Produces a canonical event happening now to `topic`; returns its id.
async fn produce_event(
    ctx: &Ctx,
    topic: &str,
    org_id: &str,
    workspace_id: &str,
    name: &str,
    properties: Value,
) -> anyhow::Result<String> {
    let event_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let event = json!({
        "envelop_version": "1.0",
        "id": event_id,
        "name": name,
        "org_id": org_id,
        "workspace_id": workspace_id,
        "anon_id": "anon-e2e",
        "occured_at": now,
        "arrived_at": now,
        "properties": properties,
    });
    ctx.kafka
        .produce(topic, "anon-e2e", event.to_string().as_bytes())
        .await?;
    Ok(event_id)
}

/// Waits until the workspace's `events_v2`, read through the server, holds
/// exactly the events `ids`.
async fn wait_for_events(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    workspace_id: &str,
    ids: &[&str],
) -> anyhow::Result<()> {
    let expected: Vec<Value> = ids.iter().map(|id| json!({ "id": id })).collect();
    let deadline = tokio::time::Instant::now() + INGEST_TIMEOUT;
    loop {
        let rows = workspace_query(
            ctx,
            token,
            org_id,
            workspace_id,
            "SELECT toString(event_id) AS id FROM events_v2 ORDER BY id",
        )
        .await?;
        let mut sorted = expected.clone();
        sorted.sort_by_key(|row| row["id"].to_string());
        if rows == Value::Array(sorted) {
            return Ok(());
        }
        ensure!(
            tokio::time::Instant::now() < deadline,
            "{workspace_id} events_v2 did not become {ids:?} within {INGEST_TIMEOUT:?}: {rows}"
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Runs `sql` through the server's `/query` endpoint for the workspace and
/// returns the result rows.
async fn workspace_query(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    workspace_id: &str,
    sql: &str,
) -> anyhow::Result<Value> {
    let response = ctx
        .post(
            "/query",
            Some(token),
            &[
                ("X-Organization-Id", org_id),
                ("X-Workspace-Id", workspace_id),
            ],
            json!({ "query": sql }),
        )
        .await?;
    expect_status(&response, 200)?;
    let body = response.json()?;
    ensure!(body["success"] == true, "query {sql:?} failed: {body}");
    Ok(body["response"]["data"].clone())
}

/// The workspace user can read neither its Kafka table (a SELECT would
/// consume from the topic) nor another database.
async fn expect_workspace_user_isolated(ctx: &Ctx, workspace_id: &str) -> anyhow::Result<()> {
    let other = format!(
        "e2e_other_{}",
        &uuid::Uuid::new_v4().simple().to_string()[..8]
    );
    ctx.clickhouse
        .query(&format!("CREATE DATABASE `{other}`"))
        .await?;
    let result = async {
        ctx.clickhouse
            .query(&format!(
                "CREATE TABLE `{other}`.t (x UInt8) ENGINE = Memory"
            ))
            .await?;
        for (label, sql) in [
            (
                "its own Kafka table",
                "SELECT * FROM events_kafka LIMIT 0".to_owned(),
            ),
            ("another database", format!("SELECT * FROM `{other}`.t")),
        ] {
            let answer = ctx
                .clickhouse
                .query_as(workspace_id, WORKSPACE_PASSWORD, Some(workspace_id), &sql)
                .await?;
            ensure!(
                answer.status != 200 && answer.body.contains("ACCESS_DENIED"),
                "workspace user could read {label}: {} {}",
                answer.status,
                answer.body.trim()
            );
        }
        Ok(())
    }
    .await;
    ctx.clickhouse
        .query(&format!("DROP DATABASE `{other}` SYNC"))
        .await?;
    result
}

/// Neither a ClickHouse database nor a ClickHouse user exists for the workspace.
async fn expect_no_workspace_database(ctx: &Ctx, workspace_id: &str) -> anyhow::Result<()> {
    let databases = ctx
        .clickhouse
        .rows(&format!(
            "SELECT name FROM system.databases WHERE name = {}",
            quote(workspace_id)
        ))
        .await?;
    ensure!(databases.is_empty(), "database {workspace_id} still exists");
    let users = ctx
        .clickhouse
        .rows(&format!(
            "SELECT name FROM system.users WHERE name = {}",
            quote(workspace_id)
        ))
        .await?;
    ensure!(
        users.is_empty(),
        "ClickHouse user {workspace_id} still exists"
    );
    Ok(())
}

/// Creates a workspace, which stays PENDING until it is provisioned.
async fn create_pending_workspace(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    name: &str,
) -> anyhow::Result<String> {
    let response = ctx
        .post(
            "/workspaces",
            Some(token),
            &[("X-Organization-Id", org_id)],
            json!({ "name": name }),
        )
        .await?;
    expect_status(&response, 201).with_context(|| format!("creating workspace {name:?}"))?;
    Ok(str_field(&response.json()?, "id")?.to_owned())
}

/// Creates a workspace and provisions it through the admin API.
async fn create_workspace(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    name: &str,
) -> anyhow::Result<String> {
    let workspace_id = create_pending_workspace(ctx, token, org_id, name).await?;
    let response = provision(&ctx.http, &workspace_id).await?;
    expect_status(&response, 202).with_context(|| format!("provisioning workspace {name:?}"))?;
    let outcome = wait_for_provisioning(ctx, &workspace_id).await?;
    ensure!(
        outcome["status"] == "ACTIVE",
        "provisioning workspace {name:?} failed: {outcome}"
    );
    Ok(workspace_id)
}

/// Asks `http`'s server to provision the workspace through the admin API.
async fn provision(http: &Http, workspace_id: &str) -> anyhow::Result<Response> {
    http.post(
        &format!("/admin/workspaces/{workspace_id}/provision"),
        None,
        &[("X-Admin-Token", ADMIN_TOKEN)],
        json!({}),
    )
    .await
}

/// Polls the admin API until the workspace is no longer CREATING; returns
/// its provisioning state.
async fn wait_for_provisioning(ctx: &Ctx, workspace_id: &str) -> anyhow::Result<Value> {
    let deadline = tokio::time::Instant::now() + PROVISION_TIMEOUT;
    loop {
        let response = ctx
            .get(
                &format!("/admin/workspaces/{workspace_id}"),
                None,
                &[("X-Admin-Token", ADMIN_TOKEN)],
            )
            .await?;
        expect_status(&response, 200)?;
        let body = response.json()?;
        if body["status"] != "CREATING" {
            return Ok(body);
        }
        ensure!(
            tokio::time::Instant::now() < deadline,
            "{workspace_id} still CREATING after {PROVISION_TIMEOUT:?}"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// Puts the workspace `CREATING` under a new attempt that started `age` ago;
/// returns the attempt.
async fn start_attempt(ctx: &Ctx, workspace_id: &str, age: Duration) -> anyhow::Result<uuid::Uuid> {
    let attempt = uuid::Uuid::new_v4();
    sqlx::query(
        "UPDATE workspaces SET status = 'CREATING', provisioning_attempt = $2, \
         provisioning_started_at = NOW() - make_interval(secs => $3) WHERE id = $1",
    )
    .bind(workspace_id)
    .bind(attempt)
    .bind(age.as_secs_f64())
    .execute(&ctx.pool)
    .await?;
    Ok(attempt)
}

async fn delete_workspace(
    ctx: &Ctx,
    token: &str,
    org_id: &str,
    workspace_id: &str,
) -> anyhow::Result<Response> {
    ctx.http
        .delete(
            "/workspace",
            Some(token),
            &[
                ("X-Organization-Id", org_id),
                ("X-Workspace-Id", workspace_id),
            ],
        )
        .await
}

async fn workspace_status(ctx: &Ctx, workspace_id: &str) -> anyhow::Result<String> {
    Ok(
        sqlx::query_scalar("SELECT status::text FROM workspaces WHERE id = $1")
            .bind(workspace_id)
            .fetch_one(&ctx.pool)
            .await?,
    )
}

async fn create_org(ctx: &Ctx, token: &str, name: &str) -> anyhow::Result<String> {
    let response = ctx
        .post("/organizations", Some(token), &[], json!({ "name": name }))
        .await?;
    expect_status(&response, 201).with_context(|| format!("creating organization {name:?}"))?;
    Ok(str_field(&response.json()?, "id")?.to_owned())
}

async fn org_role(ctx: &Ctx, sub: &str, org_id: &str) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT om.role::text FROM organization_memberships om
         JOIN users u ON u.id = om.user_id
         WHERE u.keycloak_user_id = $1 AND om.organization_id = $2",
    )
    .bind(sub)
    .bind(org_id)
    .fetch_optional(&ctx.pool)
    .await?)
}

async fn workspace_role(
    ctx: &Ctx,
    sub: &str,
    workspace_id: &str,
) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT pm.role::text FROM workspace_memberships pm
         JOIN users u ON u.id = pm.user_id
         WHERE u.keycloak_user_id = $1 AND pm.workspace_id = $2",
    )
    .bind(sub)
    .bind(workspace_id)
    .fetch_optional(&ctx.pool)
    .await?)
}

async fn expect_no_workspace(ctx: &Ctx, name: &str) -> anyhow::Result<()> {
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM workspaces WHERE name = $1")
        .bind(name)
        .fetch_one(&ctx.pool)
        .await?;
    ensure!(
        rows == 0,
        "rejected request still inserted workspace {name:?}"
    );
    Ok(())
}

/// `id` is a valid slug of the form `{base}-{6 random [a-z0-9]}`.
fn expect_generated_slug(id: &str, base: &str) -> anyhow::Result<()> {
    ensure!(is_valid_slug(id), "{id:?} is not a valid slug");
    let suffix = id
        .strip_prefix(base)
        .and_then(|rest| rest.strip_prefix('-'))
        .with_context(|| format!("{id:?} does not start with \"{base}-\""))?;
    ensure!(
        suffix.len() == SUFFIX_LEN
            && suffix
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()),
        "{id:?} does not end in a {SUFFIX_LEN}-char [a-z0-9] suffix"
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

fn expect_client_error(response: &Response) -> anyhow::Result<()> {
    ensure!(
        (400..500).contains(&response.status),
        "expected a 4xx, got {}: {}",
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
