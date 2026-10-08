use canonical_event::ProjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LiveDashboard {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub query: String,
    pub description: String,
    pub title: Option<String>,
    pub chart_config: Option<String>,
    pub last_ran_at: Option<DateTime<Utc>>,
    pub response: Option<JsonValue>,
    pub creation_source: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a new live dashboard
pub async fn create_dashboard(
    pool: &PgPool,
    project_id: &ProjectId,
    query: &str,
    description: &str,
    title: Option<&str>,
    chart_config: Option<&str>,
    creation_source: Option<&str>,
) -> Result<LiveDashboard, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, LiveDashboard>(
        r#"
        INSERT INTO live_dashboards (id, project_id, query, description, title, chart_config, creation_source)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        RETURNING id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(query)
    .bind(description)
    .bind(title)
    .bind(chart_config)
    .bind(creation_source)
    .fetch_one(pool)
    .await
}

/// Get a single live dashboard by id
pub async fn get_dashboard(
    pool: &PgPool,
    dashboard_id: Uuid,
) -> Result<Option<LiveDashboard>, sqlx::Error> {
    sqlx::query_as::<_, LiveDashboard>(
        r#"
        SELECT id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        FROM live_dashboards
        WHERE id = $1
        "#,
    )
    .bind(dashboard_id)
    .fetch_optional(pool)
    .await
}

/// Get a dashboard by project_id and creation_source (uses idx_project_id_source index)
pub async fn get_dashboard_by_source(
    pool: &PgPool,
    project_id: &ProjectId,
    creation_source: &str,
) -> Result<Option<LiveDashboard>, sqlx::Error> {
    sqlx::query_as::<_, LiveDashboard>(
        r#"
        SELECT id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        FROM live_dashboards
        WHERE project_id = $1 AND creation_source = $2
        "#,
    )
    .bind(project_id)
    .bind(creation_source)
    .fetch_optional(pool)
    .await
}

/// Get all live dashboards associated with a project
pub async fn get_dashboards(
    pool: &PgPool,
    project_id: &ProjectId,
) -> Result<Vec<LiveDashboard>, sqlx::Error> {
    sqlx::query_as::<_, LiveDashboard>(
        r#"
        SELECT id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        FROM live_dashboards
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

/// Get minimal dashboard info (id and description) for listing
pub async fn list_dashboards_summary(
    pool: &PgPool,
    project_id: &ProjectId,
) -> Result<Vec<(Uuid, String)>, sqlx::Error> {
    sqlx::query_as::<_, (Uuid, String)>(
        r#"
        SELECT id, description
        FROM live_dashboards
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

/// Update a live dashboard
pub async fn update_dashboard(
    pool: &PgPool,
    dashboard_id: Uuid,
    query: &str,
    description: &str,
    title: Option<&str>,
    chart_config: Option<&str>,
) -> Result<LiveDashboard, sqlx::Error> {
    sqlx::query_as::<_, LiveDashboard>(
        r#"
        UPDATE live_dashboards
        SET query = $2, description = $3, title = $4, chart_config = $5, updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        "#,
    )
    .bind(dashboard_id)
    .bind(query)
    .bind(description)
    .bind(title)
    .bind(chart_config)
    .fetch_one(pool)
    .await
}

/// Update the last run timestamp and response for a dashboard
pub async fn update_dashboard_run(
    pool: &PgPool,
    dashboard_id: Uuid,
    response: &JsonValue,
) -> Result<LiveDashboard, sqlx::Error> {
    sqlx::query_as::<_, LiveDashboard>(
        r#"
        UPDATE live_dashboards
        SET last_ran_at = NOW(), response = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, query, description, title, chart_config, last_ran_at, response, creation_source, created_at, updated_at
        "#,
    )
    .bind(dashboard_id)
    .bind(response)
    .fetch_one(pool)
    .await
}

/// Delete a live dashboard
pub async fn delete_dashboard(pool: &PgPool, dashboard_id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM live_dashboards
        WHERE id = $1
        "#,
    )
    .bind(dashboard_id)
    .execute(pool)
    .await?;

    Ok(())
}
