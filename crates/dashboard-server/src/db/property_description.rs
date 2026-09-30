use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PropertyDescription {
    pub id: Uuid,
    pub project_id: Uuid,
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create or update property description (upsert)
pub async fn upsert_property_description(
    pool: &PgPool,
    project_id: Uuid,
    event_name: &str,
    property_name: &str,
    property_type: &str,
    description: &str,
) -> Result<PropertyDescription, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, PropertyDescription>(
        r#"
        INSERT INTO property_descriptions (id, project_id, event_name, property_name, property_type, description)
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (project_id, event_name, property_name) DO UPDATE SET
            property_type = EXCLUDED.property_type,
            description = EXCLUDED.description,
            updated_at = NOW()
        RETURNING id, project_id, event_name, property_name, property_type, description, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(event_name)
    .bind(property_name)
    .bind(property_type)
    .bind(description)
    .fetch_one(pool)
    .await
}

/// Get a single property description by project_id, event_name, and property_name
pub async fn get_property_description(
    pool: &PgPool,
    project_id: Uuid,
    event_name: &str,
    property_name: &str,
) -> Result<Option<PropertyDescription>, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(
        r#"
        SELECT id, project_id, event_name, property_name, property_type, description, created_at, updated_at
        FROM property_descriptions
        WHERE project_id = $1 AND event_name = $2 AND property_name = $3
        "#,
    )
    .bind(project_id)
    .bind(event_name)
    .bind(property_name)
    .fetch_optional(pool)
    .await
}

/// List all property descriptions for a project
pub async fn list_property_descriptions_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<PropertyDescription>, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(
        r#"
        SELECT id, project_id, event_name, property_name, property_type, description, created_at, updated_at
        FROM property_descriptions
        WHERE project_id = $1
        ORDER BY event_name ASC, property_name ASC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

/// List property descriptions for a specific event in a project
pub async fn list_property_descriptions_by_event(
    pool: &PgPool,
    project_id: Uuid,
    event_name: &str,
) -> Result<Vec<PropertyDescription>, sqlx::Error> {
    sqlx::query_as::<_, PropertyDescription>(
        r#"
        SELECT id, project_id, event_name, property_name, property_type, description, created_at, updated_at
        FROM property_descriptions
        WHERE project_id = $1 AND event_name = $2
        ORDER BY property_name ASC
        "#,
    )
    .bind(project_id)
    .bind(event_name)
    .fetch_all(pool)
    .await
}

/// Delete a property description
pub async fn delete_property_description(
    pool: &PgPool,
    project_id: Uuid,
    event_name: &str,
    property_name: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM property_descriptions
        WHERE project_id = $1 AND event_name = $2 AND property_name = $3
        "#,
    )
    .bind(project_id)
    .bind(event_name)
    .bind(property_name)
    .execute(pool)
    .await?;

    Ok(())
}
