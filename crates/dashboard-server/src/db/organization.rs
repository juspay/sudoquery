use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Organization {
    pub id: Uuid,
    pub name: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_organization(
    pool: &PgPool,
    name: &str,
) -> Result<Organization, sqlx::Error> {
    let uuid7 = uuid7::uuid7();
    let id = Uuid::from_bytes(*uuid7.as_bytes());

    sqlx::query_as::<_, Organization>(
        r#"
        INSERT INTO organizations (id, name)
        VALUES ($1, $2)
        RETURNING id, name, deleted_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .fetch_one(pool)
    .await
}

pub async fn get_organization_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<Organization>, sqlx::Error> {
    sqlx::query_as::<_, Organization>(
        r#"
        SELECT id, name, deleted_at, created_at, updated_at
        FROM organizations
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete_organization(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE organizations
        SET deleted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn list_active_organizations(pool: &PgPool) -> Result<Vec<Organization>, sqlx::Error> {
    sqlx::query_as::<_, Organization>(
        r#"
        SELECT id, name, deleted_at, created_at, updated_at
        FROM organizations
        WHERE deleted_at IS NULL
        ORDER BY created_at DESC
        "#,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationWithAccess {
    #[sqlx(flatten)]
    pub organization: Organization,
    pub access_level: String,
}

pub async fn list_user_organizations(pool: &PgPool, user_id: Uuid) -> Result<Vec<OrganizationWithAccess>, sqlx::Error> {
    sqlx::query_as::<_, OrganizationWithAccess>(
        r#"
        SELECT DISTINCT ON (o.id)
            o.id,
            o.name,
            o.deleted_at,
            o.created_at,
            o.updated_at,
            CASE
                WHEN om.role = 'org_admin' THEN 'org_admin'
                WHEN om.role = 'org_user' THEN 'org_user'
                WHEN pm.role IS NOT NULL THEN 'project_' || pm.role
                ELSE 'none'
            END AS access_level
        FROM organizations o
        LEFT JOIN organization_memberships om ON o.id = om.organization_id AND om.user_id = $1
        LEFT JOIN projects p ON o.id = p.organization_id AND p.deleted_at IS NULL
        LEFT JOIN project_memberships pm ON p.id = pm.project_id AND pm.user_id = $1
        WHERE o.deleted_at IS NULL
          AND (om.user_id IS NOT NULL OR pm.user_id IS NOT NULL)
        ORDER BY o.id, o.created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}
