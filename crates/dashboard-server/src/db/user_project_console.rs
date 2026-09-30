use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserProjectConsole {
    pub id: Uuid,
    pub user_id: Uuid,
    pub proj_id: Uuid,
    pub console: Option<String>,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Create a new console for a user in a project
pub async fn create_console(
    pool: &PgPool,
    user_id: Uuid,
    proj_id: Uuid,
    name: Option<&str>,
    console: Option<&str>,
) -> Result<UserProjectConsole, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, UserProjectConsole>(
        r#"
        INSERT INTO user_project_consoles (id, user_id, proj_id, name, console)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, user_id, proj_id, console, name, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(proj_id)
    .bind(name)
    .bind(console)
    .fetch_one(pool)
    .await
}

/// Get a single console by id (without ownership check — use carefully)
pub async fn get_console_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<UserProjectConsole>, sqlx::Error> {
    sqlx::query_as::<_, UserProjectConsole>(
        r#"
        SELECT id, user_id, proj_id, console, name, created_at, updated_at
        FROM user_project_consoles
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

/// Get a console by id AND user_id (ownership-enforced lookup)
pub async fn get_console_by_id_and_user(
    pool: &PgPool,
    id: Uuid,
    user_id: Uuid,
) -> Result<Option<UserProjectConsole>, sqlx::Error> {
    sqlx::query_as::<_, UserProjectConsole>(
        r#"
        SELECT id, user_id, proj_id, console, name, created_at, updated_at
        FROM user_project_consoles
        WHERE id = $1 AND user_id = $2
        "#,
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ConsoleSummary {
    pub id: Uuid,
    pub name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// List all consoles for a specific user in a specific project (summary only)
pub async fn list_consoles_by_user_and_project(
    pool: &PgPool,
    user_id: Uuid,
    proj_id: Uuid,
) -> Result<Vec<ConsoleSummary>, sqlx::Error> {
    sqlx::query_as::<_, ConsoleSummary>(
        r#"
        SELECT id, name, created_at, updated_at
        FROM user_project_consoles
        WHERE user_id = $1 AND proj_id = $2
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .bind(proj_id)
    .fetch_all(pool)
    .await
}

/// Update a console's name and/or content. Only affects the console if owned by the caller.
/// Caller must verify ownership before calling (via get_console_by_id_and_user).
pub async fn update_console(
    pool: &PgPool,
    id: Uuid,
    name: Option<&str>,
    console: Option<&str>,
) -> Result<UserProjectConsole, sqlx::Error> {
    sqlx::query_as::<_, UserProjectConsole>(
        r#"
        UPDATE user_project_consoles
        SET name = $2, console = $3, updated_at = NOW()
        WHERE id = $1
        RETURNING id, user_id, proj_id, console, name, created_at, updated_at
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(console)
    .fetch_one(pool)
    .await
}

/// Delete a console by id. Caller must verify ownership before calling.
pub async fn delete_console(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM user_project_consoles
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}
