//! Collector API keys: per-project keys for the collector's authenticated
//! endpoint (`Authorization: Bearer sk_<kid>_<secret>`). The full key is
//! returned once, by the create call; only `sha256(secret)` is stored.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, db,
    middleware::{ProjectAccess, ProjectAdmin},
};

/// Longest accepted `key_name`, in characters.
const MAX_KEY_NAME_CHARS: usize = 255;

// ============ Request/Response Types ============

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateCollectorKeyRequest {
    pub key_name: String,
    /// When the key stops working; never when absent.
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct CollectorKeyResponse {
    pub kid: String,
    pub key_name: String,
    pub created_at: String,
    pub created_by: Option<String>,
    pub expires_at: Option<String>,
    pub revoked_at: Option<String>,
}

impl From<db::CollectorKeyRecord> for CollectorKeyResponse {
    fn from(record: db::CollectorKeyRecord) -> Self {
        Self {
            kid: record.kid,
            key_name: record.key_name,
            created_at: record.created_at.to_rfc3339(),
            created_by: record.created_by.map(|id| id.to_string()),
            expires_at: record.expires_at.map(|at| at.to_rfc3339()),
            revoked_at: record.revoked_at.map(|at| at.to_rfc3339()),
        }
    }
}

#[derive(Serialize)]
pub struct CreatedCollectorKeyResponse {
    #[serde(flatten)]
    pub record: CollectorKeyResponse,
    /// The full key; this response is the only place it ever appears.
    pub key: String,
}

// ============ Create Key ============

pub async fn create_collector_key(
    State(state): State<AppState>,
    ProjectAdmin { project, auth_user }: ProjectAdmin,
    Json(req): Json<CreateCollectorKeyRequest>,
) -> Result<(StatusCode, Json<CreatedCollectorKeyResponse>), CollectorKeyError> {
    let key_name = req.key_name.trim();
    if key_name.is_empty() || key_name.chars().count() > MAX_KEY_NAME_CHARS {
        return Err(CollectorKeyError::InvalidName);
    }
    if req.expires_at.is_some_and(|at| at <= Utc::now()) {
        return Err(CollectorKeyError::ExpiryInPast);
    }

    let (record, key) = db::create_collector_key(
        &state.db_pool,
        &project.id,
        key_name,
        auth_user.user.id,
        req.expires_at,
    )
    .await
    .map_err(|e| CollectorKeyError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreatedCollectorKeyResponse {
            record: record.into(),
            key: key.to_string(),
        }),
    ))
}

// ============ List Keys ============

pub async fn list_collector_keys(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> Result<Json<Vec<CollectorKeyResponse>>, CollectorKeyError> {
    let keys = db::list_collector_keys(&state.db_pool, &project.id)
        .await
        .map_err(|e| CollectorKeyError::Database(e.to_string()))?;

    Ok(Json(keys.into_iter().map(Into::into).collect()))
}

// ============ Revoke Key ============

pub async fn revoke_collector_key(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Path(kid): Path<String>,
) -> Result<Json<CollectorKeyResponse>, CollectorKeyError> {
    let record = db::revoke_collector_key(&state.db_pool, &project.id, &kid)
        .await
        .map_err(|e| CollectorKeyError::Database(e.to_string()))?
        .ok_or(CollectorKeyError::NotFound)?;

    Ok(Json(record.into()))
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum CollectorKeyError {
    #[error("key_name must be 1-255 characters")]
    InvalidName,
    #[error("expires_at must be in the future")]
    ExpiryInPast,
    #[error("Collector key not found")]
    NotFound,
    #[error("Database error: {0}")]
    Database(String),
    #[error(transparent)]
    Auth(#[from] crate::middleware::AuthError),
}

impl axum::response::IntoResponse for CollectorKeyError {
    fn into_response(self) -> axum::response::Response {
        let status = match self {
            CollectorKeyError::InvalidName | CollectorKeyError::ExpiryInPast => {
                StatusCode::BAD_REQUEST
            }
            CollectorKeyError::NotFound => StatusCode::NOT_FOUND,
            CollectorKeyError::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            CollectorKeyError::Auth(e) => return e.into_response(),
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
