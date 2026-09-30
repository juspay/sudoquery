use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct EventRow {
    #[serde(with = "clickhouse::serde::uuid")]
    pub event_id: uuid::Uuid,
    #[serde(with = "clickhouse::serde::uuid")]
    pub proj_id: uuid::Uuid,
    #[serde(default)]
    pub user_id: String,
    #[serde(default)]
    pub anon_id: String,
    pub event_name: String,
    #[serde(with = "event_timestamp_format")]
    pub event_timestamp: i64,
    #[serde(default)]
    pub properties: Value,
    #[serde(default)]
    pub device_type: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub browser: String,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub city: String,
    #[serde(default)]
    pub user_agent: String,
    #[serde(default)]
    pub version: String,
}

mod event_timestamp_format {
    use chrono::{TimeZone, Utc};
    use serde::{self, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(timestamp: &i64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let dt = Utc
            .timestamp_millis_opt(*timestamp)
            .single()
            .unwrap_or_else(|| Utc::now());
        let s = dt.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        serializer.serialize_str(&s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<i64, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = chrono::NaiveDateTime::parse_from_str(&s, "%Y-%m-%d %H:%M:%S%.3f")
            .map_err(serde::de::Error::custom)?;
        Ok(dt.and_utc().timestamp_millis())
    }
}

impl Default for EventRow {
    fn default() -> Self {
        Self {
            event_id: uuid::Uuid::new_v4(),
            proj_id: uuid::Uuid::nil(),
            user_id: String::default(),
            anon_id: String::default(),
            event_name: String::default(),
            event_timestamp: Utc::now().timestamp_millis(),
            properties: Value::Object(serde_json::Map::new()),
            device_type: String::default(),
            platform: String::default(),
            browser: String::default(),
            country: None,
            city: String::default(),
            user_agent: String::default(),
            version: String::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub r#type: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub r#type: String,
    pub function: ToolFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    #[serde(rename = "type")]
    pub r#type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}
