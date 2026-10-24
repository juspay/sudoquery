use canonical_event::{OrgId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "org_role", rename_all = "snake_case")]
pub enum OrgRole {
    OrgAdmin,
    OrgUser,
}

impl OrgRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            OrgRole::OrgAdmin => "org_admin",
            OrgRole::OrgUser => "org_user",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "workspace_role", rename_all = "snake_case")]
pub enum WorkspaceRole {
    WorkspaceAdmin,
    WorkspaceUser,
}

impl WorkspaceRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            WorkspaceRole::WorkspaceAdmin => "workspace_admin",
            WorkspaceRole::WorkspaceUser => "workspace_user",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationMembership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: OrgId,
    pub role: OrgRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct WorkspaceMembership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub workspace_id: WorkspaceId,
    pub role: WorkspaceRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Organization membership functions

pub async fn add_user_to_organization(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
    role: &OrgRole,
) -> Result<OrganizationMembership, sqlx::Error> {
    let uuid7 = uuid7::uuid7();
    let id = Uuid::from_bytes(*uuid7.as_bytes());

    sqlx::query_as::<_, OrganizationMembership>(
        r#"
        INSERT INTO organization_memberships (id, user_id, organization_id, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, organization_id, role, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(organization_id)
    .bind(role)
    .fetch_one(pool)
    .await
}

pub async fn get_user_organization_role(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
) -> Result<Option<OrgRole>, sqlx::Error> {
    let result: Option<(OrgRole,)> = sqlx::query_as(
        r#"
        SELECT role
        FROM organization_memberships
        WHERE user_id = $1 AND organization_id = $2
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await?;

    Ok(result.map(|r| r.0))
}

pub async fn get_organization_membership(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
) -> Result<Option<OrganizationMembership>, sqlx::Error> {
    sqlx::query_as::<_, OrganizationMembership>(
        r#"
        SELECT id, user_id, organization_id, role, created_at, updated_at
        FROM organization_memberships
        WHERE user_id = $1 AND organization_id = $2
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .fetch_optional(pool)
    .await
}

pub async fn remove_user_from_organization(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM organization_memberships
        WHERE user_id = $1 AND organization_id = $2
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_organization_membership_role(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: &OrgId,
    role: &OrgRole,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE organization_memberships
        SET role = $3, updated_at = NOW()
        WHERE user_id = $1 AND organization_id = $2
        "#,
    )
    .bind(user_id)
    .bind(organization_id)
    .bind(role)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_organization_members(
    pool: &PgPool,
    organization_id: &OrgId,
) -> Result<Vec<(Uuid, String, String, OrgRole)>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT u.id, u.email, u.username, om.role
        FROM users u
        JOIN organization_memberships om ON u.id = om.user_id
        WHERE om.organization_id = $1 AND u.deleted_at IS NULL
        ORDER BY u.created_at DESC
        "#,
    )
    .bind(organization_id)
    .fetch_all(pool)
    .await
}

// Workspace membership functions

pub async fn add_user_to_workspace(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: &WorkspaceId,
    role: &WorkspaceRole,
) -> Result<WorkspaceMembership, sqlx::Error> {
    let uuid7 = uuid7::uuid7();
    let id = Uuid::from_bytes(*uuid7.as_bytes());

    sqlx::query_as::<_, WorkspaceMembership>(
        r#"
        INSERT INTO workspace_memberships (id, user_id, workspace_id, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, workspace_id, role, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(workspace_id)
    .bind(role)
    .fetch_one(pool)
    .await
}

pub async fn get_user_workspace_role(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: &WorkspaceId,
) -> Result<Option<WorkspaceRole>, sqlx::Error> {
    // First check for direct workspace membership
    let result: Option<(WorkspaceRole,)> = sqlx::query_as(
        r#"
        SELECT role
        FROM workspace_memberships
        WHERE user_id = $1 AND workspace_id = $2
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;

    if result.is_some() {
        return Ok(result.map(|r| r.0));
    }

    // Check if user is org admin (grant admin role on all workspaces)
    let org_admin: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM organization_memberships om
        JOIN workspaces p ON p.organization_id = om.organization_id
        WHERE om.user_id = $1
          AND p.id = $2
          AND om.role::text = 'org_admin'
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;

    if org_admin.is_some() {
        return Ok(Some(WorkspaceRole::WorkspaceAdmin));
    }

    // Check if user is org member (grant user role on workspaces)
    let org_member: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM organization_memberships om
        JOIN workspaces p ON p.organization_id = om.organization_id
        WHERE om.user_id = $1
          AND p.id = $2
          AND om.role::text = 'org_user'
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;

    if org_member.is_some() {
        return Ok(Some(WorkspaceRole::WorkspaceUser));
    }

    Ok(None)
}

pub async fn get_workspace_membership(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: &WorkspaceId,
) -> Result<Option<WorkspaceMembership>, sqlx::Error> {
    sqlx::query_as::<_, WorkspaceMembership>(
        r#"
        SELECT id, user_id, workspace_id, role, created_at, updated_at
        FROM workspace_memberships
        WHERE user_id = $1 AND workspace_id = $2
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await
}

pub async fn remove_user_from_workspace(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: &WorkspaceId,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM workspace_memberships
        WHERE user_id = $1 AND workspace_id = $2
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_workspace_membership_role(
    pool: &PgPool,
    user_id: Uuid,
    workspace_id: &WorkspaceId,
    role: &WorkspaceRole,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE workspace_memberships
        SET role = $3, updated_at = NOW()
        WHERE user_id = $1 AND workspace_id = $2
        "#,
    )
    .bind(user_id)
    .bind(workspace_id)
    .bind(role)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_workspace_members(
    pool: &PgPool,
    workspace_id: &WorkspaceId,
) -> Result<Vec<(Uuid, String, String, WorkspaceRole)>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT u.id, u.email, u.username, pm.role
        FROM users u
        JOIN workspace_memberships pm ON u.id = pm.user_id
        WHERE pm.workspace_id = $1 AND u.deleted_at IS NULL
        ORDER BY u.created_at DESC
        "#,
    )
    .bind(workspace_id)
    .fetch_all(pool)
    .await
}

pub async fn list_user_workspaces(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<Vec<WorkspaceId>, sqlx::Error> {
    let result: Vec<(WorkspaceId,)> = sqlx::query_as(
        r#"
        SELECT workspace_id
        FROM workspace_memberships
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(result.into_iter().map(|r| r.0).collect())
}
