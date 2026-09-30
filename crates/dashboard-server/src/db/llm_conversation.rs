use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LLMConversation {
    pub id: Uuid,
    pub chat_id: Uuid,
    pub message: Uuid,
    pub created_at: DateTime<Utc>
}

