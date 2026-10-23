use canonical_event::ProjectId;
use canonical_event::collector_key::{CollectorKey, KID_BYTES, SECRET_BYTES};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::{MAX_SLUG_INSERT_ATTEMPTS, is_unique_violation};

/// A collector API key as the dashboard shows it: everything but the
/// hashed secret.
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct CollectorKeyRecord {
    pub kid: String,
    pub project_id: ProjectId,
    pub key_name: String,
    pub created_at: DateTime<Utc>,
    pub created_by: Option<Uuid>,
    pub expires_at: Option<DateTime<Utc>>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Creates a key for the project; returns its record and the full key,
/// which is never stored and cannot be recovered later.
pub async fn create_collector_key(
    pool: &PgPool,
    project_id: &ProjectId,
    key_name: &str,
    created_by: Uuid,
    expires_at: Option<DateTime<Utc>>,
) -> Result<(CollectorKeyRecord, CollectorKey), sqlx::Error> {
    let mut attempt = 0;
    loop {
        let key = CollectorKey::new(
            rand::random::<[u8; KID_BYTES]>(),
            rand::random::<[u8; SECRET_BYTES]>(),
        );
        attempt += 1;

        let inserted = sqlx::query_as::<_, CollectorKeyRecord>(
            r#"
            INSERT INTO collector_keys (kid, project_id, hashed_secret, key_name, created_by, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING kid, project_id, key_name, created_at, created_by, expires_at, revoked_at
            "#,
        )
        .bind(key.kid())
        .bind(project_id)
        .bind(key.hashed_secret().as_slice())
        .bind(key_name)
        .bind(created_by)
        .bind(expires_at)
        .fetch_one(pool)
        .await;

        match inserted {
            Ok(record) => return Ok((record, key)),
            // The random kid collided with an existing key; retry with a
            // fresh one.
            Err(err) if attempt < MAX_SLUG_INSERT_ATTEMPTS && is_unique_violation(&err) => {
                tracing::warn!(
                    "generated collector key id collided (attempt {}/{}); regenerating",
                    attempt,
                    MAX_SLUG_INSERT_ATTEMPTS
                );
            }
            Err(err) => return Err(err),
        }
    }
}

pub async fn list_collector_keys(
    pool: &PgPool,
    project_id: &ProjectId,
) -> Result<Vec<CollectorKeyRecord>, sqlx::Error> {
    sqlx::query_as::<_, CollectorKeyRecord>(
        r#"
        SELECT kid, project_id, key_name, created_at, created_by, expires_at, revoked_at
        FROM collector_keys
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

/// Revokes the project's key `kid`; revoking a revoked key keeps its first
/// revocation time. `None` when the project has no such key.
pub async fn revoke_collector_key(
    pool: &PgPool,
    project_id: &ProjectId,
    kid: &str,
) -> Result<Option<CollectorKeyRecord>, sqlx::Error> {
    sqlx::query_as::<_, CollectorKeyRecord>(
        r#"
        UPDATE collector_keys
        SET revoked_at = COALESCE(revoked_at, NOW())
        WHERE kid = $1 AND project_id = $2
        RETURNING kid, project_id, key_name, created_at, created_by, expires_at, revoked_at
        "#,
    )
    .bind(kid)
    .bind(project_id)
    .fetch_optional(pool)
    .await
}
