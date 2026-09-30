use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
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
        let (status, message) = match &self {
            KeycloakError::Api { status, message } => {
                let http_status =
                    StatusCode::from_u16(*status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                (http_status, message.clone())
            }
            _ => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
