use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct LLMChat {
    pub id: Uuid,
    pub user_id: Uuid,
    pub project_id: Uuid,
    pub title: String,
    pub version: f64,
    pub chat_type: string,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
