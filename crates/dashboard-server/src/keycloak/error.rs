use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KeycloakError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Keycloak API error {status}: {message}")]
    Api { status: u16, message: String },

    #[error("Failed to parse response: {0}")]
    Parse(String),
}

impl IntoResponse for KeycloakError {
    fn into_response(self) -> Response {
        tracing::error!("Keycloak error: {}", self);
        let (status, message) = match &self {
            KeycloakError::Api { status: 409, .. } => (StatusCode::CONFLICT, "User already exists"),
            KeycloakError::Api { status: 400, .. } => {
                (StatusCode::BAD_REQUEST, "Invalid user details")
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
