use canonical_event::WorkspaceId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventDescription {
    pub id: Uuid,
    pub workspace_id: WorkspaceId,
    pub event_name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create or update event description (upsert)
pub async fn upsert_event_description(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
    event_name: &str,
    description: &str,
) -> Result<EventDescription, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, EventDescription>(
        r#"
        INSERT INTO event_descriptions (id, workspace_id, event_name, description)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (workspace_id, event_name) DO UPDATE SET
            description = EXCLUDED.description,
            updated_at = NOW()
        RETURNING id, workspace_id, event_name, description, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(workspace_id)
    .bind(event_name)
    .bind(description)
    .fetch_one(pool)
    .await
}

/// Get a single event description by workspace_id and event_name
pub async fn get_event_description(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
    event_name: &str,
) -> Result<Option<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(
        r#"
        SELECT id, workspace_id, event_name, description, created_at, updated_at
        FROM event_descriptions
        WHERE workspace_id = $1 AND event_name = $2
        "#,
    )
    .bind(workspace_id)
    .bind(event_name)
    .fetch_optional(pool)
    .await
}

/// List all event descriptions for a workspace
pub async fn list_event_descriptions_by_workspace(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
) -> Result<Vec<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(
        r#"
        SELECT id, workspace_id, event_name, description, created_at, updated_at
        FROM event_descriptions
        WHERE workspace_id = $1
        ORDER BY event_name ASC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
}

/// Delete an event description
pub async fn delete_event_description(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
    event_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM event_descriptions
        WHERE workspace_id = $1 AND event_name = $2
        "#,
    )
    .bind(workspace_id)
    .bind(event_name)
    .execute(pool)
    .await?;

    Ok(())
}
