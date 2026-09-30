use axum::{Json, extract::State, http::StatusCode};
use axum_extra::TypedHeader;
use rdkafka::producer::FutureRecord;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db;
use crate::models::EventRow;
use crate::state::AppState;

#[derive(Deserialize, Serialize)]
pub struct SessionData {
    #[serde(default)]
    pub device_type: String,
    #[serde(default)]
    pub platform: String,
    #[serde(default)]
    pub browser: String,
    #[serde(default)]
    pub country: String,
    #[serde(default)]
    pub city: String,
    pub ip_address: Option<String>,
    #[serde(default)]
    pub user_agent: String,
}

#[derive(Deserialize, Serialize)]
pub struct ClientEvent {
    pub event_id: Uuid,
    pub event_name: String,
    pub event_timestamp: i64,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub anon_id: Option<String>,
    pub properties: String,
    #[serde(default)]
    pub version: String,
}

#[derive(Deserialize, Serialize)]
pub struct BatchPayload {
    pub session: SessionData,
    pub events: Vec<ClientEvent>,
}

#[derive(Serialize)]
struct EventSchemaEntry {
    proj_id: Uuid,
    event_name: String,
    property: String,
    #[serde(rename = "type")]
    property_type: String,
}

fn get_json_type(value: &serde_json::Value) -> &'static str {
    match value {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}

fn flatten_properties(
    obj: &serde_json::Map<String, serde_json::Value>,
    prefix: &str,
    result: &mut Vec<(String, String)>,
) {
    for (key, value) in obj {
        let property_path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{}.{}", prefix, key)
        };

        match value {
            serde_json::Value::Object(nested) => {
                flatten_properties(nested, &property_path, result);
            }
            _ => {
                result.push((property_path, get_json_type(value).to_string()));
            }
        }
    }
}

impl From<(ClientEvent, &SessionData, Uuid)> for EventRow {
    fn from((event, session, proj_id): (ClientEvent, &SessionData, Uuid)) -> Self {
        let event_timestamp_ms = if event.event_timestamp < 1_000_000_000_000 {
            event.event_timestamp * 1_000
        } else {
            event.event_timestamp
        };

        let properties: serde_json::Value = serde_json::from_str(&event.properties)
            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

        EventRow {
            event_id: event.event_id,
            proj_id,
            user_id: event.user_id.clone().unwrap_or_default(),
            anon_id: event.anon_id.clone().unwrap_or_default(),
            event_name: event.event_name,
            event_timestamp: event_timestamp_ms,
            properties,
            device_type: session.device_type.clone(),
            platform: session.platform.clone(),
            browser: session.browser.clone(),
            country: if session.country.len() == 2 {
                Some(session.country.clone())
            } else {
                None
            },
            city: session.city.clone(),
            user_agent: session.user_agent.clone(),
            version: event.version,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("Missing or invalid Authorization header")]
    MissingToken,
    #[error("Invalid project token")]
    InvalidToken,
    #[error("Project not found or deleted")]
    ProjectNotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Kafka error: {0}")]
    Kafka(String),
}

impl axum::response::IntoResponse for IngestError {
    fn into_response(self) -> axum::response::Response {
        let status = match &self {
            IngestError::MissingToken => StatusCode::UNAUTHORIZED,
            IngestError::InvalidToken => StatusCode::UNAUTHORIZED,
            IngestError::ProjectNotFound => StatusCode::NOT_FOUND,
            IngestError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            IngestError::Kafka(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

pub async fn push_batch(
    State(s): State<AppState>,
    TypedHeader(auth_header): TypedHeader<AuthorizationHeader>,
    Json(payload): Json<BatchPayload>,
) -> Result<Json<serde_json::Value>, IngestError> {
    let token_str = auth_header
        .0
        .strip_prefix("Bearer ")
        .ok_or(IngestError::MissingToken)?;

    let token = Uuid::parse_str(token_str).map_err(|_| IngestError::InvalidToken)?;

    let project_row = db::get_project_by_token(&s.db_pool, token)
        .await
        .map_err(|e| IngestError::Database(e.to_string()))?
        .ok_or(IngestError::InvalidToken)?;

    if project_row.project_deleted_at.is_some() {
        return Err(IngestError::ProjectNotFound);
    }

    db::update_token_last_used(&s.db_pool, token)
        .await
        .map_err(|e| IngestError::Database(e.to_string()))?;

    tracing::info!(
        project_id = %project_row.project_id,
        organization_id = ?project_row.organization_id,
        events_count = payload.events.len(),
        "Processing batch"
    );

    let event_rows: Vec<EventRow> = payload
        .events
        .into_iter()
        .map(|event| EventRow::from((event, &payload.session, project_row.project_id)))
        .collect();

    if event_rows.is_empty() {
        return Ok(Json(json!({
            "events_sent": 0,
            "warning": "No events to send"
        })));
    }

    let mut events_sent = 0;
    for row in &event_rows {
        let payload = serde_json::to_string(row).map_err(|e| IngestError::Kafka(e.to_string()))?;
        let key = row.event_id.to_string();

        let record = FutureRecord::to("events").payload(&payload).key(&key);

        s.kafka_producer
            .send(record, std::time::Duration::from_secs(5))
            .await
            .map_err(|(e, _)| IngestError::Kafka(e.to_string()))?;

        if let serde_json::Value::Object(props) = &row.properties {
            let mut flattened = Vec::new();
            flatten_properties(props, "", &mut flattened);

            for (property_path, property_type) in flattened {
                let schema_entry = EventSchemaEntry {
                    proj_id: row.proj_id,
                    event_name: row.event_name.clone(),
                    property: property_path,
                    property_type,
                };

                let schema_payload = serde_json::to_string(&schema_entry)
                    .map_err(|e| IngestError::Kafka(e.to_string()))?;

                let schema_key = format!("{}:{}", row.proj_id, row.event_name);
                let schema_record = FutureRecord::to("event_schema_catalog")
                    .payload(&schema_payload)
                    .key(&schema_key);

                let _ = s
                    .kafka_producer
                    .send(schema_record, std::time::Duration::from_secs(5))
                    .await;
            }
        }

        events_sent += 1;
    }

    tracing::info!(
        events_sent = events_sent,
        project_id = %project_row.project_id,
        "Batch sent to Kafka"
    );

    Ok(Json(json!({
        "events_sent": events_sent
    })))
}

pub async fn push_batches(
    State(s): State<AppState>,
    TypedHeader(auth_header): TypedHeader<AuthorizationHeader>,
    Json(payload): Json<Vec<BatchPayload>>,
) -> Result<Json<serde_json::Value>, IngestError> {
    let token_str = auth_header
        .0
        .strip_prefix("Bearer ")
        .ok_or(IngestError::MissingToken)?;

    let token = Uuid::parse_str(token_str).map_err(|_| IngestError::InvalidToken)?;

    let project_row = db::get_project_by_token(&s.db_pool, token)
        .await
        .map_err(|e| IngestError::Database(e.to_string()))?
        .ok_or(IngestError::InvalidToken)?;

    if project_row.project_deleted_at.is_some() {
        return Err(IngestError::ProjectNotFound);
    }

    db::update_token_last_used(&s.db_pool, token)
        .await
        .map_err(|e| IngestError::Database(e.to_string()))?;

    let total_events: usize = payload.iter().map(|batch| batch.events.len()).sum();
    tracing::info!(
        project_id = %project_row.project_id,
        organization_id = ?project_row.organization_id,
        batch_count = payload.len(),
        events_count = total_events,
        "Processing batches"
    );

    let mut event_rows: Vec<EventRow> = Vec::new();
    for batch in payload {
        for event in batch.events {
            event_rows.push(EventRow::from((
                event,
                &batch.session,
                project_row.project_id,
            )));
        }
    }

    if event_rows.is_empty() {
        return Ok(Json(json!({
            "events_sent": 0,
            "warning": "No events to send"
        })));
    }

    let mut events_sent = 0;
    for row in &event_rows {
        let payload = serde_json::to_string(row).map_err(|e| IngestError::Kafka(e.to_string()))?;
        let key = row.event_id.to_string();

        let record = FutureRecord::to("events").payload(&payload).key(&key);

        s.kafka_producer
            .send(record, std::time::Duration::from_secs(5))
            .await
            .map_err(|(e, _)| IngestError::Kafka(e.to_string()))?;

        if let serde_json::Value::Object(props) = &row.properties {
            let mut flattened = Vec::new();
            flatten_properties(props, "", &mut flattened);

            for (property_path, property_type) in flattened {
                let schema_entry = EventSchemaEntry {
                    proj_id: row.proj_id,
                    event_name: row.event_name.clone(),
                    property: property_path,
                    property_type,
                };

                let schema_payload = serde_json::to_string(&schema_entry)
                    .map_err(|e| IngestError::Kafka(e.to_string()))?;

                let schema_key = format!("{}:{}", row.proj_id, row.event_name);
                let schema_record = FutureRecord::to("event_schema_catalog")
                    .payload(&schema_payload)
                    .key(&schema_key);

                let _ = s
                    .kafka_producer
                    .send(schema_record, std::time::Duration::from_secs(5))
                    .await;
            }
        }

        events_sent += 1;
    }

    tracing::info!(
        events_sent = events_sent,
        project_id = %project_row.project_id,
        "Batches sent to Kafka"
    );

    Ok(Json(json!({
        "events_sent": events_sent
    })))
}

pub struct AuthorizationHeader(String);

impl axum_extra::headers::Header for AuthorizationHeader {
    fn name() -> &'static axum::http::HeaderName {
        static NAME: axum::http::HeaderName = axum::http::HeaderName::from_static("authorization");
        &NAME
    }

    fn decode<'i, I>(values: &mut I) -> Result<Self, axum_extra::headers::Error>
    where
        I: Iterator<Item = &'i axum::http::HeaderValue>,
    {
        let value = values
            .next()
            .ok_or_else(axum_extra::headers::Error::invalid)?;
        let s = value
            .to_str()
            .map_err(|_| axum_extra::headers::Error::invalid())?;
        Ok(AuthorizationHeader(s.to_string()))
    }

    fn encode<E: Extend<axum::http::HeaderValue>>(&self, _values: &mut E) {
        // Not needed for extraction
    }
}
