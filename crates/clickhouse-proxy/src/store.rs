//! The proxy's own Postgres tables (migrations/): saved SQL consoles and
//! event and property descriptions, per workspace.

use std::str::FromStr;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use uuid::Uuid;

/// Connects with `schema` first on the search path, creating the schema if
/// needed, then brings it up to date with the migrations. Tables and the
/// migration history all live in that schema.
pub async fn connect(database_url: &str, schema: &str) -> Result<PgPool, String> {
    let valid_schema = schema
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_lowercase() || b == b'_')
        && schema
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !valid_schema {
        return Err(format!(
            "DATABASE_SCHEMA must be lowercase letters, digits and `_`, got {schema:?}"
        ));
    }
    let options = PgConnectOptions::from_str(database_url)
        .map_err(|e| format!("DATABASE_URL: {e}"))?
        .options([("search_path", schema)]);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .map_err(|e| format!("connecting to Postgres: {e}"))?;
    sqlx::query(&format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\""))
        .execute(&pool)
        .await
        .map_err(|e| format!("creating schema {schema}: {e}"))?;
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("migrating schema {schema}: {e}"))?;
    Ok(pool)
}

fn new_id() -> Uuid {
    Uuid::now_v7()
}

// ============ Consoles ============

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Console {
    pub id: Uuid,
    pub workspace_id: String,
    pub owner: String,
    pub name: Option<String>,
    pub console: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ConsoleSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const CONSOLE_COLUMNS: &str = "id, workspace_id, owner, name, console, created_at, updated_at";

pub async fn create_console(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
    name: Option<&str>,
    console: Option<&str>,
) -> Result<Console, sqlx::Error> {
    sqlx::query_as::<_, Console>(&format!(
        "INSERT INTO consoles (id, workspace_id, owner, name, console)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING {CONSOLE_COLUMNS}"
    ))
    .bind(new_id())
    .bind(workspace_id)
    .bind(owner)
    .bind(name)
    .bind(console)
    .fetch_one(pool)
    .await
}

/// The owner's consoles in the workspace, newest first.
pub async fn list_consoles(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
) -> Result<Vec<ConsoleSummary>, sqlx::Error> {
    sqlx::query_as::<_, ConsoleSummary>(
        "SELECT id, name, created_at, updated_at FROM consoles
         WHERE workspace_id = $1 AND owner = $2
         ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .bind(owner)
    .fetch_all(pool)
    .await
}

/// One console, only if it is the owner's and in the workspace.
pub async fn get_console(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
    id: Uuid,
) -> Result<Option<Console>, sqlx::Error> {
    sqlx::query_as::<_, Console>(&format!(
        "SELECT {CONSOLE_COLUMNS} FROM consoles WHERE id = $1 AND workspace_id = $2 AND owner = $3"
    ))
    .bind(id)
    .bind(workspace_id)
    .bind(owner)
    .fetch_optional(pool)
    .await
}

/// Changes the name and/or text of the owner's console; a `None` keeps the
/// current value. `None` when there is no such console.
pub async fn update_console(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
    id: Uuid,
    name: Option<&str>,
    console: Option<&str>,
) -> Result<Option<Console>, sqlx::Error> {
    sqlx::query_as::<_, Console>(&format!(
        "UPDATE consoles
         SET name = COALESCE($4, name), console = COALESCE($5, console), updated_at = now()
         WHERE id = $1 AND workspace_id = $2 AND owner = $3
         RETURNING {CONSOLE_COLUMNS}"
    ))
    .bind(id)
    .bind(workspace_id)
    .bind(owner)
    .bind(name)
    .bind(console)
    .fetch_optional(pool)
    .await
}

/// Whether the owner's console existed and was deleted.
pub async fn delete_console(
    pool: &PgPool,
    workspace_id: &str,
    owner: &str,
    id: Uuid,
) -> Result<bool, sqlx::Error> {
    let result =
        sqlx::query("DELETE FROM consoles WHERE id = $1 AND workspace_id = $2 AND owner = $3")
            .bind(id)
            .bind(workspace_id)
            .bind(owner)
            .execute(pool)
            .await?;
    Ok(result.rows_affected() > 0)
}

// ============ Event descriptions ============

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EventDescription {
    pub id: Uuid,
    pub workspace_id: String,
    pub event_name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const EVENT_COLUMNS: &str = "id, workspace_id, event_name, description, created_at, updated_at";

pub async fn upsert_event_description(
    pool: &PgPool,
    workspace_id: &str,
    event_name: &str,
    description: &str,
) -> Result<EventDescription, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(&format!(
        "INSERT INTO event_descriptions (id, workspace_id, event_name, description)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (workspace_id, event_name) DO UPDATE SET
             description = EXCLUDED.description,
             updated_at = now()
         RETURNING {EVENT_COLUMNS}"
    ))
    .bind(new_id())
    .bind(workspace_id)
    .bind(event_name)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn get_event_description(
    pool: &PgPool,
    workspace_id: &str,
    event_name: &str,
) -> Result<Option<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(&format!(
        "SELECT {EVENT_COLUMNS} FROM event_descriptions WHERE workspace_id = $1 AND event_name = $2"
    ))
    .bind(workspace_id)
    .bind(event_name)
    .fetch_optional(pool)
    .await
}

pub async fn list_event_descriptions(
    pool: &PgPool,
    workspace_id: &str,
) -> Result<Vec<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(&format!(
        "SELECT {EVENT_COLUMNS} FROM event_descriptions WHERE workspace_id = $1 ORDER BY event_name ASC"
    ))
    .bind(workspace_id)
    .fetch_all(pool)
    .await
}

// ============ Property descriptions ============

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PropertyDescription {
    pub id: Uuid,
    pub workspace_id: String,
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

const PROPERTY_COLUMNS: &str = "id, workspace_id, event_name, property_name, property_type, description, created_at, updated_at";

pub async fn upsert_property_description(
    pool: &PgPool,
    workspace_id: &str,
    event_name: &str,
    property_name: &str,
    property_type: &str,
    description: &str,
) -> Result<PropertyDescription, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(&format!(
        "INSERT INTO property_descriptions (id, workspace_id, event_name, property_name, property_type, description)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (workspace_id, event_name, property_name) DO UPDATE SET
             property_type = EXCLUDED.property_type,
             description = EXCLUDED.description,
             updated_at = now()
         RETURNING {PROPERTY_COLUMNS}"
    ))
    .bind(new_id())
    .bind(workspace_id)
    .bind(event_name)
    .bind(property_name)
    .bind(property_type)
    .bind(description)
    .fetch_one(pool)
    .await
}

pub async fn get_property_description(
    pool: &PgPool,
    workspace_id: &str,
    event_name: &str,
    property_name: &str,
) -> Result<Option<PropertyDescription>, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(&format!(
        "SELECT {PROPERTY_COLUMNS} FROM property_descriptions
         WHERE workspace_id = $1 AND event_name = $2 AND property_name = $3"
    ))
    .bind(workspace_id)
    .bind(event_name)
    .bind(property_name)
    .fetch_optional(pool)
    .await
}

/// The workspace's property descriptions, or only one event's.
pub async fn list_property_descriptions(
    pool: &PgPool,
    workspace_id: &str,
    event_name: Option<&str>,
) -> Result<Vec<PropertyDescription>, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(&format!(
        "SELECT {PROPERTY_COLUMNS} FROM property_descriptions
         WHERE workspace_id = $1 AND ($2::text IS NULL OR event_name = $2)
         ORDER BY event_name ASC, property_name ASC"
    ))
    .bind(workspace_id)
    .bind(event_name)
    .fetch_all(pool)
    .await
}
