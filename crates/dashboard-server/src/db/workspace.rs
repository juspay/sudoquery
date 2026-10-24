use std::time::Duration;

use canonical_event::{OrgId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use super::{MAX_SLUG_INSERT_ATTEMPTS, is_unique_violation};
use crate::ids;

/// Where a workspace is in provisioning its infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "workspace_status", rename_all = "UPPERCASE")]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkspaceStatus {
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
pub struct Workspace {
    pub id: WorkspaceId,
    pub organization_id: Option<OrgId>,
    pub name: String,
    pub timezone: Option<String>,
    pub status: WorkspaceStatus,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceToken {
    pub id: Uuid,
    pub workspace_id: WorkspaceId,
    pub token: Uuid,
    pub name: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceWithToken {
    pub id: WorkspaceId,
    pub organization_id: Option<OrgId>,
    pub name: String,
    pub status: WorkspaceStatus,
    pub workspace_token: Uuid,
    pub created_at: DateTime<Utc>,
}

pub async fn create_workspace(
    pool: &PgPool,
    organization_id: Option<&OrgId>,
    name: &str,
) -> Result<WorkspaceWithToken, sqlx::Error> {
    let mut attempt = 0;
    loop {
        let id = ids::generate_workspace_id(name);
        let token_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
        let workspace_token = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
        attempt += 1;

        let mut tx = pool.begin().await?;

        let inserted = sqlx::query(
            r#"
            INSERT INTO workspaces (id, organization_id, name, status)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(&id)
        .bind(organization_id)
        .bind(name)
        .bind(WorkspaceStatus::Pending)
        .execute(&mut *tx)
        .await;

        if let Err(err) = inserted {
            // A unique violation means the random slug suffix collided with
            // an existing workspace; dropping `tx` rolls the transaction back
            // and the loop retries with a fresh suffix.
            if attempt < MAX_SLUG_INSERT_ATTEMPTS && is_unique_violation(&err) {
                tracing::warn!(
                    "generated workspace slug '{}' collided (attempt {}/{}); regenerating suffix",
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
            INSERT INTO workspace_tokens (id, workspace_id, token, name)
            VALUES ($1, $2, $3, 'default')
            "#,
        )
        .bind(token_id)
        .bind(&id)
        .bind(workspace_token)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        return Ok(WorkspaceWithToken {
            id,
            organization_id: organization_id.cloned(),
            name: name.to_string(),
            status: WorkspaceStatus::Pending,
            workspace_token,
            created_at: Utc::now(),
        });
    }
}

pub async fn get_workspace_by_id(
    pool: &PgPool,
    id: &WorkspaceId,
) -> Result<Option<Workspace>, sqlx::Error> {
    sqlx::query_as::<_, Workspace>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
        FROM workspaces
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_workspace_by_id_and_organization(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
    organization_id: &OrgId,
) -> Result<Option<Workspace>, sqlx::Error> {
    sqlx::query_as::<_, Workspace>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
        FROM workspaces
        WHERE id = $1 AND organization_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(workspace_id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_workspaces_by_organization(
    pool: &PgPool,
    organization_id: &OrgId,
) -> Result<Vec<Workspace>, sqlx::Error> {
    sqlx::query_as::<_, Workspace>(
        r#"
        SELECT id, organization_id, name, timezone, status, deleted_at, created_at, updated_at
        FROM workspaces
        WHERE organization_id = $1 AND deleted_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
}

/// List workspaces user has access to within a specific organization
/// (direct workspace membership OR org membership)
pub async fn list_user_workspaces_in_organization(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
) -> Result<Vec<Workspace>, sqlx::Error> {
    sqlx::query_as::<_, Workspace>(
        r#"
        SELECT DISTINCT p.id, p.organization_id, p.name, p.timezone, p.status, p.deleted_at, p.created_at, p.updated_at
        FROM workspaces p
        WHERE p.organization_id = $1
          AND p.deleted_at IS NULL
          AND (
              -- Direct workspace membership
              EXISTS (
                  SELECT 1 FROM workspace_memberships pm
                  WHERE pm.workspace_id = p.id AND pm.user_id = $2
              )
              -- OR org membership (any role grants workspace access)
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

pub async fn soft_delete_workspace(pool: &PgPool, id: &WorkspaceId) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE workspaces
        SET deleted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

/// A workspace's provisioning state, as the admin API reports it.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct WorkspaceProvisioning {
    pub id: WorkspaceId,
    pub organization_id: Option<OrgId>,
    pub status: WorkspaceStatus,
    /// Why the last attempt failed, while the workspace is `ERRORED`.
    pub provisioning_error: Option<JsonValue>,
    /// When the last attempt started.
    pub provisioning_started_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

/// The outcome of asking to provision a workspace.
#[derive(Debug)]
pub enum ProvisioningStart {
    /// The workspace is now `CREATING`; provision it under `org_id`, and
    /// record the outcome as `attempt`.
    Started {
        org_id: OrgId,
        attempt: Uuid,
    },
    NotFound,
    /// Workspaces without an organization have no topic to provision.
    NoOrganization,
    /// The workspace's organization was deleted.
    OrganizationDeleted,
    /// The workspace is `ACTIVE` or `DELETING`, or another attempt is still
    /// `CREATING` it.
    Refused(WorkspaceStatus),
}

/// The outcome of asking to delete a workspace.
#[derive(Debug)]
pub enum DeletionStart {
    /// The workspace is now `DELETING`; drop its infrastructure.
    Started,
    NotFound,
    /// An attempt is still `CREATING` it.
    BeingProvisioned,
}

pub async fn get_workspace_provisioning(
    pool: &PgPool,
    id: &WorkspaceId,
) -> Result<Option<WorkspaceProvisioning>, sqlx::Error> {
    sqlx::query_as::<_, WorkspaceProvisioning>(
        r#"
        SELECT id, organization_id, status, provisioning_error, provisioning_started_at, updated_at
        FROM workspaces
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Moves the workspace to `CREATING` under a new attempt, from `PENDING` or
/// `ERRORED`, or from a `CREATING` whose attempt started more than
/// `stale_after` ago (it died with its server). Atomic, so only one caller
/// starts each attempt; workspaces of deleted organizations are refused.
pub async fn start_provisioning(
    pool: &PgPool,
    id: &WorkspaceId,
    stale_after: Duration,
) -> Result<ProvisioningStart, sqlx::Error> {
    let attempt = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let started: Option<OrgId> = sqlx::query_scalar(
        r#"
        UPDATE workspaces
        SET status = 'CREATING',
            provisioning_attempt = $3,
            provisioning_started_at = NOW(),
            provisioning_error = NULL,
            updated_at = NOW()
        WHERE id = $1
          AND deleted_at IS NULL
          AND EXISTS (
              SELECT 1 FROM organizations o
              WHERE o.id = workspaces.organization_id AND o.deleted_at IS NULL
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

    let workspace: Option<(WorkspaceStatus, Option<OrgId>, bool)> = sqlx::query_as(
        r#"
        SELECT p.status, p.organization_id, EXISTS (
            SELECT 1 FROM organizations o
            WHERE o.id = p.organization_id AND o.deleted_at IS NULL
        )
        FROM workspaces p
        WHERE p.id = $1 AND p.deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(match workspace {
        None => ProvisioningStart::NotFound,
        Some((_, None, _)) => ProvisioningStart::NoOrganization,
        Some((_, Some(_), false)) => ProvisioningStart::OrganizationDeleted,
        Some((status, Some(_), true)) => ProvisioningStart::Refused(status),
    })
}

/// Records how `attempt` ended: `ACTIVE`, or `ERRORED` with `error`.
///
/// Returns `false`, recording nothing, when `attempt` no longer owns the
/// workspace: a newer attempt took it over, or deletion started.
pub async fn finish_provisioning(
    pool: &PgPool,
    id: &WorkspaceId,
    attempt: Uuid,
    status: WorkspaceStatus,
    error: Option<JsonValue>,
) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE workspaces
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

/// Moves the workspace to `DELETING`, unless an attempt that started less than
/// `stale_after` ago is still `CREATING` it. Atomic, and `start_provisioning`
/// never takes a `DELETING` workspace, so provisioning cannot recreate what
/// deletion drops. A `DELETING` workspace can be deleted again.
pub async fn start_deleting(
    pool: &PgPool,
    id: &WorkspaceId,
    organization_id: &OrgId,
    stale_after: Duration,
) -> Result<DeletionStart, sqlx::Error> {
    let started = sqlx::query(
        r#"
        UPDATE workspaces
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
        match get_workspace_by_id_and_organization(pool, id, organization_id).await? {
            None => DeletionStart::NotFound,
            Some(_) => DeletionStart::BeingProvisioned,
        },
    )
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceWithTokenRow {
    pub workspace_id: WorkspaceId,
    pub organization_id: Option<OrgId>,
    pub workspace_name: String,
    pub workspace_deleted_at: Option<DateTime<Utc>>,
    pub workspace_created_at: DateTime<Utc>,
    pub workspace_updated_at: DateTime<Utc>,
    pub token_id: Uuid,
    pub token: Uuid,
    pub token_name: Option<String>,
    pub token_last_used_at: Option<DateTime<Utc>>,
    pub token_created_at: DateTime<Utc>,
}

pub async fn get_workspace_by_token(
    pool: &PgPool,
    token: Uuid,
) -> Result<Option<WorkspaceWithTokenRow>, sqlx::Error> {
    sqlx::query_as::<_, WorkspaceWithTokenRow>(
        r#"
        SELECT
            p.id AS workspace_id,
            p.organization_id,
            p.name AS workspace_name,
            p.deleted_at AS workspace_deleted_at,
            p.created_at AS workspace_created_at,
            p.updated_at AS workspace_updated_at,
            pt.id AS token_id,
            pt.token,
            pt.name AS token_name,
            pt.last_used_at AS token_last_used_at,
            pt.created_at AS token_created_at
        FROM workspaces p
        JOIN workspace_tokens pt ON p.id = pt.workspace_id
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
        UPDATE workspace_tokens
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
    workspace_id: &WorkspaceId,
    name: &str,
) -> Result<WorkspaceToken, sqlx::Error> {
    let token_id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());
    let workspace_token = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, WorkspaceToken>(
        r#"
        INSERT INTO workspace_tokens (id, workspace_id, token, name)
        VALUES ($1, $2, $3, $4)
        RETURNING id, workspace_id, token, name, last_used_at, created_at
        "#,
    )
    .bind(token_id)
    .bind(workspace_id)
    .bind(workspace_token)
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn list_tokens_by_workspace(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
) -> Result<Vec<WorkspaceToken>, sqlx::Error> {
    sqlx::query_as::<_, WorkspaceToken>(
        r#"
        SELECT id, workspace_id, token, name, last_used_at, created_at
        FROM workspace_tokens
        WHERE workspace_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_token(pool: &PgPool, token: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM workspace_tokens
        WHERE token = $1
        "#,
    )
    .bind(token)
    .execute(pool)
    .await?;

    Ok(())
}
