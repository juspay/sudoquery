use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub keycloak_user_id: String,
    pub email: String,
    pub username: String,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn create_user(
    pool: &PgPool,
    keycloak_user_id: &str,
    email: &str,
    username: &str,
) -> Result<User, sqlx::Error> {
    let uuid7 = uuid7::uuid7();
    let id = Uuid::from_bytes(*uuid7.as_bytes());

    sqlx::query_as::<_, User>(
        r#"
        INSERT INTO users (id, keycloak_user_id, email, username)
        VALUES ($1, $2, $3, $4)
        RETURNING id, keycloak_user_id, email, username, deleted_at, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(keycloak_user_id)
    .bind(email)
    .bind(username)
    .fetch_one(pool)
    .await
}

pub async fn get_user_by_id(pool: &PgPool, id: Uuid) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, keycloak_user_id, email, username, deleted_at, created_at, updated_at
        FROM users
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_keycloak_id(
    pool: &PgPool,
    keycloak_user_id: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, keycloak_user_id, email, username, deleted_at, created_at, updated_at
        FROM users
        WHERE keycloak_user_id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(keycloak_user_id)
    .fetch_optional(pool)
    .await
}

pub async fn get_user_by_email(pool: &PgPool, email: &str) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>(
        r#"
        SELECT id, keycloak_user_id, email, username, deleted_at, created_at, updated_at
        FROM users
        WHERE email = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await
}

pub async fn soft_delete_user(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET deleted_at = NOW(), updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_user_email(
    pool: &PgPool,
    id: Uuid,
    email: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET email = $2, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(email)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_user_username(
    pool: &PgPool,
    id: Uuid,
    username: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        UPDATE users
        SET username = $2, updated_at = NOW()
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(username)
    .execute(pool)
    .await?;

    Ok(())
}
