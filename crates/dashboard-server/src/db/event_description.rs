use canonical_event::ProjectId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EventDescription {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub event_name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create or update event description (upsert)
pub async fn upsert_event_description(
    pool: &PgPool,
    project_id: &ProjectId,
    event_name: &str,
    description: &str,
) -> Result<EventDescription, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, EventDescription>(
        r#"
        INSERT INTO event_descriptions (id, project_id, event_name, description)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (project_id, event_name) DO UPDATE SET
            description = EXCLUDED.description,
            updated_at = NOW()
        RETURNING id, project_id, event_name, description, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(event_name)
    .bind(description)
    .fetch_one(pool)
    .await
}

/// Get a single event description by project_id and event_name
pub async fn get_event_description(
    pool: &PgPool,
    project_id: &ProjectId,
    event_name: &str,
) -> Result<Option<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(
        r#"
        SELECT id, project_id, event_name, description, created_at, updated_at
        FROM event_descriptions
        WHERE project_id = $1 AND event_name = $2
        "#,
    )
    .bind(project_id)
    .bind(event_name)
    .fetch_optional(pool)
    .await
}

/// List all event descriptions for a project
pub async fn list_event_descriptions_by_project(
    pool: &PgPool,
    project_id: &ProjectId,
) -> Result<Vec<EventDescription>, sqlx::Error> {
    sqlx::query_as::<_, EventDescription>(
        r#"
        SELECT id, project_id, event_name, description, created_at, updated_at
        FROM event_descriptions
        WHERE project_id = $1
        ORDER BY event_name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

/// Delete an event description
pub async fn delete_event_description(
    pool: &PgPool,
    project_id: &ProjectId,
    event_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM event_descriptions
        WHERE project_id = $1 AND event_name = $2
        "#,
    )
    .bind(project_id)
    .bind(event_name)
    .execute(pool)
    .await?;

    Ok(())
}
