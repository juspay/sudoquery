use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub message: JsonValue,
    pub created_at: DateTime<Utc>,
    pub role: String,
}

pub async fn create_message(
    pool: &PgPool,
    chat_id: Uuid,
    message: &JsonValue,
    role: &str,
) -> Result<Message, sqlx::Error> {
    let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

    sqlx::query_as::<_, Message>(
        r#"
        INSERT INTO messages (id, chat_id, message, role)
        VALUES ($1, $2, $3, $4)
        RETURNING id, chat_id, message, created_at, role
        "#,
    )
    .bind(id)
    .bind(chat_id)
    .bind(message)
    .bind(role)
    .fetch_one(pool)
    .await
}

pub async fn get_message_by_id(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<Message>, sqlx::Error> {
    sqlx::query_as::<_, Message>(
        r#"
        SELECT id, chat_id, message, created_at, role
        FROM messages
        WHERE id = $1
        "#,
    )
    .bind(id)
    .fetch_optional(pool)
    .await
}

pub async fn list_messages_by_chat(
    pool: &PgPool,
    chat_id: Uuid,
) -> Result<Vec<Message>, sqlx::Error> {
    sqlx::query_as::<_, Message>(
        r#"
        SELECT id, chat_id, message, created_at, role
        FROM messages
        WHERE chat_id = $1
        ORDER BY created_at ASC
        "#,
    )
    .bind(chat_id)
    .fetch_all(pool)
    .await
}

pub async fn delete_message(pool: &PgPool, id: Uuid) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        DELETE FROM messages
        WHERE id = $1
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn insert_messages(
    pool: &PgPool,
    chat_id: Uuid,
    messages: &[(JsonValue, DateTime<Utc>, String)],
) -> Result<Vec<Message>, sqlx::Error> {
    let mut inserted = Vec::with_capacity(messages.len());

    for (message, created_at, role) in messages {
        let id = Uuid::from_bytes(*uuid7::uuid7().as_bytes());

        let msg = sqlx::query_as::<_, Message>(
            r#"
            INSERT INTO messages (id, chat_id, message, created_at, role)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, chat_id, message, created_at, role
            "#,
        )
        .bind(id)
        .bind(chat_id)
        .bind(message)
        .bind(created_at)
        .bind(role)
        .fetch_one(pool)
        .await?;

        inserted.push(msg);
    }

    Ok(inserted)
}

pub async fn update_message_content(
    pool: &PgPool,
    id: Uuid,
    message: &JsonValue,
) -> Result<Message, sqlx::Error> {
    sqlx::query_as::<_, Message>(
        r#"
        UPDATE messages
        SET message = $2, updated_at = NOW()
        WHERE id = $1
        RETURNING id, chat_id, message, created_at, role
        "#,
    )
    .bind(id)
    .bind(message)
    .fetch_one(pool)
    .await
}

pub async fn delete_messages_after(
    pool: &PgPool,
    chat_id: Uuid,
    message_id: Uuid,
) -> Result<(), sqlx::Error> {
    // Get the created_at timestamp of the specified message
    let created_at = sqlx::query_scalar::<_, DateTime<Utc>>(
        r#"
        SELECT created_at FROM messages WHERE id = $1
        "#,
    )
    .bind(message_id)
    .fetch_one(pool)
    .await?;

    println!("created_at {}", created_at);

    // Delete all messages after this timestamp in the same chat
    sqlx::query(
        r#"
        DELETE FROM messages
        WHERE chat_id = $1 AND created_at > $2
        "#,
    )
    .bind(chat_id)
    .bind(created_at)
    .execute(pool)
    .await?;

    Ok(())
}
