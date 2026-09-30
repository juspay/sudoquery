use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "invitation_type")]
#[sqlx(rename_all = "snake_case")]
pub enum InvitationType {
    Organization,
    Project,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "invitation_status")]
#[sqlx(rename_all = "snake_case")]
pub enum InvitationStatus {
    Pending,
    Accepted,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Invitation {
    pub id: Uuid,
    pub email: String,
    pub invitation_type: InvitationType,
    pub target_id: Uuid,
    pub role: String,
    pub invited_by: Uuid,
    pub status: InvitationStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

pub async fn create_invitation(
    pool: &PgPool,
    email: &str,
    invitation_type: InvitationType,
    target_id: Uuid,
    role: &str,
    invited_by: Uuid,
) -> Result<Invitation, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, Invitation>(
        r#"
        INSERT INTO invitations (id, email, invitation_type, target_id, role, invited_by, status)
        VALUES ($1, $2, $3, $4, $5, $6, 'pending')
        ON CONFLICT (email, target_id) WHERE status = 'pending' DO UPDATE SET
            role = EXCLUDED.role,
            invited_by = EXCLUDED.invited_by,
            updated_at = NOW(),
            expires_at = NOW() + INTERVAL '7 days'
        RETURNING id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        "#,
    )
    .bind(id)
    .bind(email)
    .bind(invitation_type)
    .bind(target_id)
    .bind(role)
    .bind(invited_by)
    .fetch_one(pool)
    .await
}

pub async fn get_invitation_by_id(
    pool: &PgPool,
    invitation_id: Uuid,
) -> Result<Option<Invitation>, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        SELECT id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        FROM invitations
        WHERE id = $1
        "#,
    )
    .bind(invitation_id)
    .fetch_optional(pool)
    .await
}

pub async fn list_invitations_for_user(
    pool: &PgPool,
    email: &str,
) -> Result<Vec<Invitation>, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        SELECT id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        FROM invitations
        WHERE email = $1 AND status = 'pending' AND expires_at > NOW()
        ORDER BY created_at DESC
        "#,
    )
    .bind(email)
    .fetch_all(pool)
    .await
}

pub async fn list_invitations_by_inviter(
    pool: &PgPool,
    invited_by: Uuid,
) -> Result<Vec<Invitation>, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        SELECT id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        FROM invitations
        WHERE invited_by = $1 AND status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(invited_by)
    .fetch_all(pool)
    .await
}

pub async fn list_invitations_for_target(
    pool: &PgPool,
    target_id: Uuid,
    invitation_type: InvitationType,
) -> Result<Vec<Invitation>, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        SELECT id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        FROM invitations
        WHERE target_id = $1 AND invitation_type = $2 AND status = 'pending'
        ORDER BY created_at DESC
        "#,
    )
    .bind(target_id)
    .bind(invitation_type)
    .fetch_all(pool)
    .await
}

pub async fn accept_invitation(
    pool: &PgPool,
    invitation_id: Uuid,
) -> Result<Invitation, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        UPDATE invitations
        SET status = 'accepted', updated_at = NOW()
        WHERE id = $1 AND status = 'pending' AND expires_at > NOW()
        RETURNING id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        "#,
    )
    .bind(invitation_id)
    .fetch_one(pool)
    .await
}

pub async fn revoke_invitation(
    pool: &PgPool,
    invitation_id: Uuid,
) -> Result<Invitation, sqlx::Error> {
    sqlx::query_as::<_, Invitation>(
        r#"
        UPDATE invitations
        SET status = 'revoked', updated_at = NOW()
        WHERE id = $1 AND status = 'pending'
        RETURNING id, email, invitation_type, target_id, role, invited_by, status, created_at, updated_at, expires_at
        "#,
    )
    .bind(invitation_id)
    .fetch_one(pool)
    .await
}

pub async fn mark_expired_invitations(pool: &PgPool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE invitations
        SET status = 'expired', updated_at = NOW()
        WHERE status = 'pending' AND expires_at <= NOW()
        "#,
    )
    .execute(pool)
    .await?;

    Ok(result.rows_affected())
}
