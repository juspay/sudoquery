use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Chat {
    pub id: Uuid,
    pub project_id: Uuid,
    pub user_id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub version: i32,
    pub chat_type: ChatType,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialEq, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "chat_type", rename_all = "snake_case")]
pub enum ChatType {
    General,
    CreateDashboard,
}

pub async fn create_chat(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
    title: &str,
    chat_type: &ChatType,
) -> Result<Chat, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, Chat>(
        r#"
        INSERT INTO chats (id, project_id, user_id, title, version, chat_type)
        VALUES ($1, $2, $3, $4, 1, $5)
        RETURNING id, project_id, user_id, title, created_at, version, chat_type
        "#,
    )
    .bind(id)
    .bind(project_id)
    .bind(user_id)
    .bind(title)
    .bind(chat_type)
    .fetch_one(pool)
    .await
}

pub async fn get_chat_by_id(pool: &PgPool, id: Uuid) -> Result<Option<Chat>, sqlx::Error> {
    sqlx::query_as::<_, Chat>(
        r#"
        SELECT id, project_id, user_id, title, created_at, version, chat_type
        FROM chats
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_chats_by_project(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Vec<Chat>, sqlx::Error> {
    sqlx::query_as::<_, Chat>(
        r#"
        SELECT id, project_id, user_id, title, created_at, version, chat_type
        FROM chats
        WHERE project_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .fetch_all(pool)
    .await
}

pub async fn list_chats_by_user(pool: &PgPool, user_id: Uuid) -> Result<Vec<Chat>, sqlx::Error> {
    sqlx::query_as::<_, Chat>(
        r#"
        SELECT id, project_id, user_id, title, created_at, version, chat_type
        FROM chats
        WHERE user_id = $1
        ORDER BY created_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn list_chats_by_project_and_user(
    pool: &PgPool,
    project_id: Uuid,
    user_id: Uuid,
) -> Result<Vec<Chat>, sqlx::Error> {
    sqlx::query_as::<_, Chat>(
        r#"
        SELECT id, project_id, user_id, title, created_at, version, chat_type
        FROM chats
        WHERE project_id = $1 AND user_id = $2 AND version = 1
        ORDER BY created_at DESC
        "#,
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_chat(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM chats
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn update_chat_title(pool: &PgPool, id: Uuid, title: &str) -> Result<Chat, sqlx::Error> {
    sqlx::query_as::<_, Chat>(
        r#"
        UPDATE chats
        SET title = $2
        WHERE id = $1
        RETURNING id, project_id, user_id, title, created_at, version, chat_type
        "#,
    )
    .bind(id)
    .bind(title)
    .fetch_one(pool)
    .await
}
