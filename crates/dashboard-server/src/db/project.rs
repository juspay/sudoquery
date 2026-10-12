use std::time::Duration;

use canonical_event::{OrgId, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use super::{MAX_SLUG_INSERT_ATTEMPTS, is_unique_violation};
use crate::ids;

/// Where a project is in provisioning its infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "project_status", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum ProjectStatus {
    /// Created from the dashboard; nothing is provisioned yet.
    Pending,
    /// The admin API is provisioning it.
    Creating,
    /// Provisioning stopped at a step; see `provisioning_error`.
    Errored,
    /// Everything is provisioned.
    Active,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Project {
    pub id: ProjectId,
    pub organization_id: Option<OrgId>,
    pub name: String,
    pub timezone: Option<String>,
    pub status: ProjectStatus,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectToken {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub token: Uuid,
    pub name: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectWithToken {
    pub id: ProjectId,
    pub organization_id: Option<OrgId>,
    pub name: String,
    pub status: ProjectStatus,
    pub project_token: Uuid,
    pub created_at: DateTime<Utc>,
}

pub async fn create_project(
    pool: &PgPool,
    organization_id: Option<&OrgId>,
    name: &str,
) -> Result<ProjectWithToken, sqlx::Error> {
    let mut attempt = 0;
    loop {
        let id = ids::generate_project_id(name);
        let token_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
        let project_token = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
        attempt += 1;

        let mut tx = pool.begin().await?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO projects (id, organization_id, name, status)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&id)
        .bind(organization_id)
        .bind(name)
        .bind(ProjectStatus::Pending)
        .execute(&mut *tx)
        .await;

        if let Err(err) = inserted {
            // A unique violation means the random slug suffix collided with
            // an existing project; dropping `tx` rolls the transaction back
            // and the loop retries with a fresh suffix.
            if attempt < MAX_SLUG_INSERT_ATTEMPTS && is_unique_violation(&err) {
                tracing::warn!(
                    "generated project slug '{}' collided (attempt {}/{}); regenerating suffix",
                    id,
                    attempt,
                    MAX_SLUG_INSERT_ATTEMPTS
                );
                continue;
            }
            return Err(err);
        }

        sqlx::query(
            r#"
            INSERT INTO project_tokens (id, project_id, token, name)
            VALUES ($1, $2, $3, 'default')
            "#,
        )
        .bind(token_id)
        .bind(&id)
        .bind(project_token)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        return Ok(ProjectWithToken {
            id,
            organization_id: organization_id.cloned(),
            name: name.to_string(),
            status: ProjectStatus::Pending,
            project_token,
            created_at: Utc::now(),
        });
    }
}

pub async fn get_project_by_id(
    pool: &PgPool,
    id: &ProjectId,
) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
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
    project_id: &ProjectId,
    organization_id: &OrgId,
) -> Result<Option<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
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
    organization_id: &OrgId,
) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
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
    organization_id: &OrgId,
) -> Result<Vec<Project>, sqlx::Error> {
    sqlx::query_as::<_, Project>(
        r#"
        SELECT DISTINCT p.id, p.organization_id, p.name, p.timezone, p.status, p.deleted_at, p.created_at, p.updated_at
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

pub async fn soft_delete_project(pool: &PgPool, id: &ProjectId) -> Result<(), sqlx::Error> {
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

/// A project's provisioning state, as the admin API reports it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ProjectProvisioning {
    pub id: ProjectId,
    pub organization_id: Option<OrgId>,
    pub status: ProjectStatus,
    /// Why the last attempt failed, while the project is `ERRORED`.
    pub provisioning_error: Option<JsonValue>,
    pub updated_at: DateTime<Utc>,
}

/// The outcome of asking to provision a project.
#[derive(Debug)]
pub enum ProvisioningStart {
    /// The project is now `CREATING`; provision it under this organization.
    Started(OrgId),
    NotFound,
    /// Projects without an organization have no topic to provision.
    NoOrganization,
    /// The project is `ACTIVE`, or another attempt is still `CREATING` it.
    Refused(ProjectStatus),
}

pub async fn get_project_provisioning(
    pool: &PgPool,
    id: &ProjectId,
) -> Result<Option<ProjectProvisioning>, sqlx::Error> {
    sqlx::query_as::<_, ProjectProvisioning>(
        r#"
        SELECT id, organization_id, status, provisioning_error, updated_at
        FROM projects
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Moves the project to `CREATING`, from `PENDING` or `ERRORED`, or from a
/// `CREATING` that has not changed for `stale_after` (its attempt died with
/// the server). Atomic, so only one caller starts each attempt.
pub async fn start_provisioning(
    pool: &PgPool,
    id: &ProjectId,
    stale_after: Duration,
) -> Result<ProvisioningStart, sqlx::Error> {
    let started: Option<OrgId> = sqlx::query_scalar(
        r#"
        UPDATE projects
        SET status = 'CREATING', provisioning_error = NULL, updated_at = NOW()
        WHERE id = $1
          AND deleted_at IS NULL
          AND organization_id IS NOT NULL
          AND (
              status IN ('PENDING', 'ERRORED')
              OR (status = 'CREATING' AND updated_at < NOW() - make_interval(secs => $2))
          )
        RETURNING organization_id
        "#,
    )
    .bind(id)
    .bind(stale_after.as_secs_f64())
    .fetch_optional(pool)
    .await?;
    if let Some(org_id) = started {
        return Ok(ProvisioningStart::Started(org_id));
    }

    Ok(match get_project_provisioning(pool, id).await? {
        None => ProvisioningStart::NotFound,
        Some(project) if project.organization_id.is_none() => ProvisioningStart::NoOrganization,
        Some(project) => ProvisioningStart::Refused(project.status),
    })
}

/// Records how an attempt ended: `ACTIVE`, or `ERRORED` with `error`.
pub async fn finish_provisioning(
    pool: &PgPool,
    id: &ProjectId,
    status: ProjectStatus,
    error: Option<JsonValue>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE projects
        SET status = $2, provisioning_error = $3, updated_at = NOW()
        WHERE id = $1 AND status = 'CREATING'
        "#,
    )
    .bind(id)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectWithTokenRow {
    pub project_id: ProjectId,
    pub organization_id: Option<OrgId>,
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
    project_id: &ProjectId,
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
    project_id: &ProjectId,
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
