//! Org and project creation against a running dashboard-server.

use anyhow::{Context, bail, ensure};
use canonical_event::is_valid_slug;
use serde_json::{Value, json};
use sqlx::PgPool;

use super::db::TestDb;
use super::mock::{Mock, TestUser};
use super::server::Service;
use super::{Http, Response, run_cases, select};

const PACKAGE: &str = "hyper-analytics-dashboard-server";

/// Length of the random suffix appended to every generated slug.
const SUFFIX_LEN: usize = 6;

/// Everything a dashboard case can touch: the running server, its database
/// and the dummy OIDC / ClickHouse endpoints.
pub(crate) struct Ctx {
    http: Http,
    pool: PgPool,
    mock: Mock,
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
}

/// Builds dashboard-server, creates a throwaway database, runs the selected
/// cases and drops the database again. Returns whether every case passed.
pub(crate) async fn run(filter: Option<&str>) -> anyhow::Result<bool> {
    let cases = select(CASES, filter)?;

    // Build first: a failed build exits the process, which must not leak a database.
    Service::build(PACKAGE);
    let db = TestDb::create().await?;
    let outcome = run_with_db(&db, &cases).await;
    db.drop_database().await?;
    outcome
}

async fn run_with_db(db: &TestDb, cases: &[&super::Case<Ctx>]) -> anyhow::Result<bool> {
    let mock = Mock::start().await?;
    let server = Service::start(
        PACKAGE,
        "DASHBOARD_BIND_ADDR",
        None,
        &[
            ("DATABASE_URL", db.url.clone()),
            ("KEYCLOAK_URL", mock.keycloak_url().to_owned()),
            ("KEYCLOAK_REALM", super::mock::REALM.to_owned()),
            ("KEYCLOAK_ADMIN_USER", "e2e-admin".to_owned()),
            ("KEYCLOAK_ADMIN_PASS", "e2e-admin".to_owned()),
            ("CLICKHOUSE_URL", mock.clickhouse_url()),
            ("CLICKHOUSE_ADMIN_URL", mock.clickhouse_url()),
            ("RUST_LOG", "warn".to_owned()),
        ],
    )
    .await?;
    let ctx = Ctx {
        http: Http::new(format!("{}/api", server.base_url())),
        pool: db.pool.clone(),
        mock,
    };

    Ok(run_cases(
        &ctx,
        cases,
        |ctx| ctx.mock.reset(),
        || server.print_log_tail(),
    )
    .await)
}

cases![Ctx:
    org_creation_returns_slug_and_admin_membership,
    org_ids_are_slugified_from_names,
    org_same_name_gets_distinct_ids,
    org_rejects_client_supplied_id,
    org_creation_requires_valid_token,
    project_creation_returns_slug_token_and_clickhouse_user,
    project_creation_requires_org_admin,
    project_creation_validates_org_header,
    project_creation_rolls_back_when_clickhouse_fails,
    project_rejects_client_supplied_id,
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

// ============ Projects ============

async fn project_creation_returns_slug_token_and_clickhouse_user(ctx: &Ctx) -> anyhow::Result<()> {
    let user = ctx.user("Paula Project");
    let token = ctx.token(&user);
    let org_id = create_org(ctx, &token, "Project Home").await?;

    let response = ctx
        .post(
            "/projects",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Web Shop" }),
        )
        .await?;
    expect_status(&response, 201)?;
    let body = response.json()?;
    let project_id = str_field(&body, "id")?;
    expect_generated_slug(project_id, "web-shop")?;
    ensure!(
        body["organization_id"] == org_id.as_str(),
        "unexpected org in {body}"
    );
    let project_token: uuid::Uuid = str_field(&body, "project_token")?
        .parse()
        .context("project_token is not a UUID")?;

    let (stored_org, deleted): (Option<String>, bool) = sqlx::query_as(
        "SELECT organization_id, deleted_at IS NOT NULL FROM projects WHERE id = $1",
    )
    .bind(project_id)
    .fetch_one(&ctx.pool)
    .await
    .context("project row missing")?;
    ensure!(
        stored_org.as_deref() == Some(org_id.as_str()),
        "stored org {stored_org:?}"
    );
    ensure!(!deleted, "project is soft-deleted");

    let tokens: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM project_tokens WHERE project_id = $1 AND token = $2",
    )
    .bind(project_id)
    .bind(project_token)
    .fetch_one(&ctx.pool)
    .await?;
    ensure!(
        tokens == 1,
        "expected the returned token to be stored, found {tokens}"
    );

    let role = project_role(ctx, &user.sub, project_id).await?;
    ensure!(
        role.as_deref() == Some("project_admin"),
        "creator role is {role:?}"
    );

    let queries = ctx.mock.clickhouse_queries();
    ensure!(
        queries.len() == 2
            && queries[0].starts_with(&format!("CREATE USER '{project_id}' "))
            && queries[1] == format!("GRANT project_user TO '{project_id}'"),
        "unexpected ClickHouse queries: {queries:#?}"
    );

    let listed = ctx
        .get("/projects", Some(&token), &[("X-Organization-Id", &org_id)])
        .await?;
    expect_status(&listed, 200)?;
    let listed = listed.json()?;
    ensure!(
        listed
            .as_array()
            .is_some_and(|projects| projects.iter().any(|p| p["id"] == project_id)),
        "{project_id} missing from GET /projects: {listed}"
    );
    Ok(())
}

async fn project_creation_requires_org_admin(ctx: &Ctx) -> anyhow::Result<()> {
    let owner = ctx.token(&ctx.user("Org Owner"));
    let org_id = create_org(ctx, &owner, "Guarded Org").await?;
    let outsider = ctx.token(&ctx.user("Outsider"));

    let response = ctx
        .post(
            "/projects",
            Some(&outsider),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Sneaky Project" }),
        )
        .await?;
    expect_status(&response, 403)?;
    expect_no_project(ctx, "Sneaky Project").await
}

async fn project_creation_validates_org_header(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Header Tester"));
    let body = json!({ "name": "Headerless Project" });

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
            .post("/projects", Some(&token), &headers, body.clone())
            .await?;
        expect_status(&response, status).with_context(|| label)?;
    }
    expect_no_project(ctx, "Headerless Project").await
}

async fn project_creation_rolls_back_when_clickhouse_fails(ctx: &Ctx) -> anyhow::Result<()> {
    let user = ctx.user("Unlucky");
    let token = ctx.token(&user);
    let org_id = create_org(ctx, &token, "Flaky Warehouse").await?;
    ctx.mock.fail_clickhouse(true);

    let response = ctx
        .post(
            "/projects",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Doomed Project" }),
        )
        .await?;
    expect_status(&response, 500)?;

    let rows: Vec<(String, bool)> = sqlx::query_as(
        "SELECT id, deleted_at IS NOT NULL FROM projects WHERE organization_id = $1",
    )
    .bind(&org_id)
    .fetch_all(&ctx.pool)
    .await?;
    let [(project_id, deleted)] = rows.as_slice() else {
        bail!("expected exactly one (soft-deleted) project, found {rows:?}");
    };
    ensure!(*deleted, "project {project_id} was not soft-deleted");

    let role = project_role(ctx, &user.sub, project_id).await?;
    ensure!(role.is_none(), "failed project still granted {role:?}");
    Ok(())
}

async fn project_rejects_client_supplied_id(ctx: &Ctx) -> anyhow::Result<()> {
    let token = ctx.token(&ctx.user("Project Id Picker"));
    let org_id = create_org(ctx, &token, "Id Picker Org").await?;

    let response = ctx
        .post(
            "/projects",
            Some(&token),
            &[("X-Organization-Id", &org_id)],
            json!({ "name": "Picked Id Project", "id": "my-chosen-project" }),
        )
        .await?;
    expect_client_error(&response)?;
    ensure!(
        ctx.mock.clickhouse_queries().is_empty(),
        "rejected request still reached ClickHouse"
    );
    expect_no_project(ctx, "Picked Id Project").await
}

// ============ Helpers ============

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

async fn project_role(ctx: &Ctx, sub: &str, project_id: &str) -> anyhow::Result<Option<String>> {
    Ok(sqlx::query_scalar(
        "SELECT pm.role::text FROM project_memberships pm
         JOIN users u ON u.id = pm.user_id
         WHERE u.keycloak_user_id = $1 AND pm.project_id = $2",
    )
    .bind(sub)
    .bind(project_id)
    .fetch_optional(&ctx.pool)
    .await?)
}

async fn expect_no_project(ctx: &Ctx, name: &str) -> anyhow::Result<()> {
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM projects WHERE name = $1")
        .bind(name)
        .fetch_one(&ctx.pool)
        .await?;
    ensure!(
        rows == 0,
        "rejected request still inserted project {name:?}"
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
