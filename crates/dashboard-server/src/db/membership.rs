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
#[sqlx(type_name = "project_role", rename_all = "snake_case")]
pub enum ProjectRole {
    ProjectAdmin,
    ProjectUser,
}

impl ProjectRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProjectRole::ProjectAdmin => "project_admin",
            ProjectRole::ProjectUser => "project_user",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationMembership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub organization_id: Uuid,
    pub role: OrgRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ProjectMembership {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub role: ProjectRole,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// Organization membership functions

pub async fn add_user_to_organization(
    pool: &PgPool,
    user_id: Uuid,
    organization_id: Uuid,
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
    organization_id: Uuid,
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
    organization_id: Uuid,
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
    organization_id: Uuid,
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
    organization_id: Uuid,
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
    organization_id: Uuid,
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

// Project membership functions

pub async fn add_user_to_project(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
    role: &ProjectRole,
) -> Result<ProjectMembership, sqlx::Error> {
    let uuid7 = uuid7::uuid7();
    let id = Uuid::from_bytes(*uuid7.as_bytes());

    sqlx::query_as::<_, ProjectMembership>(
        r#"
        INSERT INTO project_memberships (id, user_id, project_id, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, user_id, project_id, role, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(project_id)
    .bind(role)
    .fetch_one(pool)
    .await
}

pub async fn get_user_project_role(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<Option<ProjectRole>, sqlx::Error> {
    // First check for direct project membership
    let result: Option<(ProjectRole,)> = sqlx::query_as(
        r#"
        SELECT role
        FROM project_memberships
        WHERE user_id = $1 AND project_id = $2
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    if result.is_some() {
        return Ok(result.map(|r| r.0));
    }

    // Check if user is org admin (grant admin role on all projects)
    let org_admin: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM organization_memberships om
        JOIN projects p ON p.organization_id = om.organization_id
        WHERE om.user_id = $1
          AND p.id = $2
          AND om.role::text = 'org_admin'
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    if org_admin.is_some() {
        return Ok(Some(ProjectRole::ProjectAdmin));
    }

    // Check if user is org member (grant user role on projects)
    let org_member: Option<(i32,)> = sqlx::query_as(
        r#"
        SELECT 1
        FROM organization_memberships om
        JOIN projects p ON p.organization_id = om.organization_id
        WHERE om.user_id = $1
          AND p.id = $2
          AND om.role::text = 'org_user'
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    if org_member.is_some() {
        return Ok(Some(ProjectRole::ProjectUser));
    }

    Ok(None)
}

pub async fn get_project_membership(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<Option<ProjectMembership>, sqlx::Error> {
    sqlx::query_as::<_, ProjectMembership>(
        r#"
        SELECT id, user_id, project_id, role, created_at, updated_at
        FROM project_memberships
        WHERE user_id = $1 AND project_id = $2
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await
}

pub async fn remove_user_from_project(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM project_memberships
        WHERE user_id = $1 AND project_id = $2
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_project_membership_role(
    pool: &PgPool,
    user_id: Uuid,
    project_id: Uuid,
    role: &ProjectRole,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE project_memberships
        SET role = $3, updated_at = NOW()
        WHERE user_id = $1 AND project_id = $2
        "#,
    )
    .bind(user_id)
    .bind(project_id)
    .bind(role)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_project_members(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<(Uuid, String, String, ProjectRole)>, sqlx::Error> {
    sqlx::query_as(
        r#"
        SELECT u.id, u.email, u.username, pm.role
        FROM users u
        JOIN project_memberships pm ON u.id = pm.user_id
        WHERE pm.project_id = $1 AND u.deleted_at IS NULL
        ORDER BY u.created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn list_user_projects(pool: &PgPool, user_id: Uuid) -> Result<Vec<Uuid>, sqlx::Error> {
    let result: Vec<(Uuid,)> = sqlx::query_as(
        r#"
        SELECT project_id
        FROM project_memberships
        WHERE user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(result.into_iter().map(|r| r.0).collect())
}
