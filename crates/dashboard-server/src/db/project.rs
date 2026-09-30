use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub timezone: Option<String>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectToken {
    pub id: Uuid,
    pub project_id: Uuid,
    pub token: Uuid,
    pub name: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectWithToken {
    pub id: Uuid,
    pub organization_id: Option<Uuid>,
    pub name: String,
    pub project_token: Uuid,
    pub created_at: DateTime<Utc>,
}

pub async fn create_project(
    pool: &PgPool,
    organization_id: Option<Uuid>,
    name: &str,
) -> Result<ProjectWithToken, sqlx::Error> {
    let project_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let token_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let project_token = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO projects (id, organization_id, name)
        VALUES ($1, $2, $3)
        "#,
    )
    .bind(project_id)
    .bind(organization_id)
    .bind(name)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO project_tokens (id, project_id, token, name)
        VALUES ($1, $2, $3, 'default')
        "#,
    )
    .bind(token_id)
    .bind(project_id)
    .bind(project_token)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let created_at = Utc::now();

    Ok(ProjectWithToken {
        id: project_id,
        organization_id,
        name: name.to_string(),
        project_token,
        created_at,
    })
}

pub async fn get_project_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, deleted_at, created_at, updated_at
        FROM projects
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_project_by_id_and_organization(
    pool: &PgPool,
    project_id: Uuid,
    organization_id: Uuid,
) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, deleted_at, created_at, updated_at
        FROM projects
        WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(project_id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_projects_by_organization(
    pool: &PgPool,
    organization_id: Uuid,
) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, deleted_at, created_at, updated_at
        FROM projects
        WHERE organization_id = $1 AND deleted_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
}

/// List projects user has access to within a specific organization
/// (direct project membership OR org membership)
pub async fn list_user_projects_in_organization(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: Uuid,
) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT DISTINCT p.id, p.organization_id, p.name, p.timezone, p.deleted_at, p.created_at, p.updated_at
        FROM projects p
        WHERE p.organization_id = $1
          AND p.deleted_at IS NULL
          AND (
              -- Direct project membership
              EXISTS (
                  SELECT 1 FROM project_memberships pm
                  WHERE pm.project_id = p.id AND pm.user_id = $2
              )
              -- OR org membership (any role grants project access)
              OR EXISTS (
                  SELECT 1 FROM organization_memberships om
                  WHERE om.organization_id = p.organization_id AND om.user_id = $2
              )
          )
        ORDER BY p.created_at DESC
        "#,
    )
    .bind(organization_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn soft_delete_project(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE projects
        SET deleted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectWithTokenRow {
    pub project_id: Uuid,
    pub organization_id: Option<Uuid>,
    pub project_name: String,
    pub project_deleted_at: Option<DateTime<Utc>>,
    pub project_created_at: DateTime<Utc>,
    pub project_updated_at: DateTime<Utc>,
    pub token_id: Uuid,
    pub token: Uuid,
    pub token_name: Option<String>,
    pub token_last_used_at: Option<DateTime<Utc>>,
    pub token_created_at: DateTime<Utc>,
}

pub async fn get_project_by_token(
    pool: &PgPool,
    token: Uuid,
) -> Result<Option<ProjectWithTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, ProjectWithTokenRow>(
        r#"
        SELECT
            p.id AS project_id,
            p.organization_id,
            p.name AS project_name,
            p.deleted_at AS project_deleted_at,
            p.created_at AS project_created_at,
            p.updated_at AS project_updated_at,
            pt.id AS token_id,
            pt.token,
            pt.name AS token_name,
            pt.last_used_at AS token_last_used_at,
            pt.created_at AS token_created_at
        FROM projects p
        JOIN project_tokens pt ON p.id = pt.project_id
        WHERE pt.token = $1 AND p.deleted_at IS NULL
        "#,
    )
    .bind(token)
    .fetch_optional(pool)
    .await
}

pub async fn update_token_last_used(pool: &PgPool, token: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE project_tokens
        SET last_used_at = NOW()
        WHERE token = $1
        "#,
    )
    .bind(token)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn create_additional_token(
    pool: &PgPool,
    project_id: Uuid,
    name: &str,
) -> Result<ProjectToken, sqlx::Error> {
    let token_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let project_token = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, ProjectToken>(
        r#"
        INSERT INTO project_tokens (id, project_id, token, name)
        VALUES ($1, $2, $3, $4)
        RETURNING id, project_id, token, name, last_used_at, created_at
        "#,
    )
    .bind(token_id)
    .bind(project_id)
    .bind(project_token)
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn list_tokens_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<ProjectToken>, sqlx::Error> {
    sqlx::query_as::<_, ProjectToken>(
        r#"
        SELECT id, project_id, token, name, last_used_at, created_at
        FROM project_tokens
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_token(pool: &PgPool, token: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM project_tokens
        WHERE token = $1
        "#,
    )
    .bind(token)
    .execute(pool)
    .await?;

    Ok(())
}
