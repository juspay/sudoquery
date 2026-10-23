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
    /// Deletion started; it drops the infrastructure and can be retried.
    Deleting,
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

pub async fn create_project(
    pool: &PgPool,
    organization_id: Option<&OrgId>,
    name: &str,
) -> Result<Project, sqlx::Error> {
    let mut attempt = 0;
    loop {
        let id = ids::generate_project_id(name);
        attempt += 1;

        let inserted = sqlx::query_as::<_, Project>(
            r#"
            INSERT INTO projects (id, organization_id, name, status)
            VALUES ($1, $2, $3, $4)
            RETURNING id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
            "#,
        )
        .bind(&id)
        .bind(organization_id)
        .bind(name)
        .bind(ProjectStatus::Pending)
        .fetch_one(pool)
        .await;

        match inserted {
            // A unique violation means the random slug suffix collided with
            // an existing project; the loop retries with a fresh suffix.
            Err(err) if attempt < MAX_SLUG_INSERT_ATTEMPTS && is_unique_violation(&err) => {
                tracing::warn!(
                    "generated project slug '{}' collided (attempt {}/{}); regenerating suffix",
                    id,
                    attempt,
                    MAX_SLUG_INSERT_ATTEMPTS
                );
            }
            result => return result,
        }
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
    /// When the last attempt started.
    pub provisioning_started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// The outcome of asking to provision a project.
#[derive(Debug)]
pub enum ProvisioningStart {
    /// The project is now `CREATING`; provision it under `org_id`, and
    /// record the outcome as `attempt`.
    Started {
        org_id: OrgId,
        attempt: Uuid,
    },
    NotFound,
    /// Projects without an organization have no topic to provision.
    NoOrganization,
    /// The project's organization was deleted.
    OrganizationDeleted,
    /// The project is `ACTIVE` or `DELETING`, or another attempt is still
    /// `CREATING` it.
    Refused(ProjectStatus),
}

/// The outcome of asking to delete a project.
#[derive(Debug)]
pub enum DeletionStart {
    /// The project is now `DELETING`; drop its infrastructure.
    Started,
    NotFound,
    /// An attempt is still `CREATING` it.
    BeingProvisioned,
}

pub async fn get_project_provisioning(
    pool: &PgPool,
    id: &ProjectId,
) -> Result<Option<ProjectProvisioning>, sqlx::Error> {
    sqlx::query_as::<_, ProjectProvisioning>(
        r#"
        SELECT id, organization_id, status, provisioning_error, provisioning_started_at, updated_at
        FROM projects
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Moves the project to `CREATING` under a new attempt, from `PENDING` or
/// `ERRORED`, or from a `CREATING` whose attempt started more than
/// `stale_after` ago (it died with its server). Atomic, so only one caller
/// starts each attempt; projects of deleted organizations are refused.
pub async fn start_provisioning(
    pool: &PgPool,
    id: &ProjectId,
    stale_after: Duration,
) -> Result<ProvisioningStart, sqlx::Error> {
    let attempt = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let started: Option<OrgId> = sqlx::query_scalar(
        r#"
        UPDATE projects
        SET status = 'CREATING',
            provisioning_attempt = $3,
            provisioning_started_at = NOW(),
            provisioning_error = NULL,
            updated_at = NOW()
        WHERE id = $1
          AND deleted_at IS NULL
          AND EXISTS (
              SELECT 1 FROM organizations o
              WHERE o.id = projects.organization_id AND o.deleted_at IS NULL
          )
          AND (
              status IN ('PENDING', 'ERRORED')
              OR (
                  status = 'CREATING'
                  AND (
                      provisioning_started_at IS NULL
                      OR provisioning_started_at < NOW() - make_interval(secs => $2)
                  )
              )
          )
        RETURNING organization_id
        "#,
    )
    .bind(id)
    .bind(stale_after.as_secs_f64())
    .bind(attempt)
    .fetch_optional(pool)
    .await?;
    if let Some(org_id) = started {
        return Ok(ProvisioningStart::Started { org_id, attempt });
    }

    let project: Option<(ProjectStatus, Option<OrgId>, bool)> = sqlx::query_as(
        r#"
        SELECT p.status, p.organization_id, EXISTS (
            SELECT 1 FROM organizations o
            WHERE o.id = p.organization_id AND o.deleted_at IS NULL
        )
        FROM projects p
        WHERE p.id = $1 AND p.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(match project {
        None => ProvisioningStart::NotFound,
        Some((_, None, _)) => ProvisioningStart::NoOrganization,
        Some((_, Some(_), false)) => ProvisioningStart::OrganizationDeleted,
        Some((status, Some(_), true)) => ProvisioningStart::Refused(status),
    })
}

/// Records how `attempt` ended: `ACTIVE`, or `ERRORED` with `error`.
///
/// Returns `false`, recording nothing, when `attempt` no longer owns the
/// project: a newer attempt took it over, or deletion started.
pub async fn finish_provisioning(
    pool: &PgPool,
    id: &ProjectId,
    attempt: Uuid,
    status: ProjectStatus,
    error: Option<JsonValue>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE projects
        SET status = $3, provisioning_error = $4, updated_at = NOW()
        WHERE id = $1
          AND provisioning_attempt = $2
          AND status = 'CREATING'
          AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(attempt)
    .bind(status)
    .bind(error)
    .execute(pool)
    .await?;

    Ok(result.rows_affected() == 1)
}

/// Moves the project to `DELETING`, unless an attempt that started less than
/// `stale_after` ago is still `CREATING` it. Atomic, and `start_provisioning`
/// never takes a `DELETING` project, so provisioning cannot recreate what
/// deletion drops. A `DELETING` project can be deleted again.
pub async fn start_deleting(
    pool: &PgPool,
    id: &ProjectId,
    organization_id: &OrgId,
    stale_after: Duration,
) -> Result<DeletionStart, sqlx::Error> {
    let started = sqlx::query(
        r#"
        UPDATE projects
        SET status = 'DELETING', updated_at = NOW()
        WHERE id = $1
          AND organization_id = $2
          AND deleted_at IS NULL
          AND (
              status <> 'CREATING'
              OR provisioning_started_at IS NULL
              OR provisioning_started_at < NOW() - make_interval(secs => $3)
          )
        "#,
    )
    .bind(id)
    .bind(organization_id)
    .bind(stale_after.as_secs_f64())
    .execute(pool)
    .await?;
    if started.rows_affected() == 1 {
        return Ok(DeletionStart::Started);
    }

    Ok(
        match get_project_by_id_and_organization(pool, id, organization_id).await? {
            None => DeletionStart::NotFound,
            Some(_) => DeletionStart::BeingProvisioned,
        },
    )
}
