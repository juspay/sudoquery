//! The admin API authenticates with one shared token: `ADMIN_API_TOKEN`,
//! sent as the `X-Admin-Token` header. While the variable is unset, every
//! admin request is refused.

use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use subtle::ConstantTimeEq;

use crate::AppState;

pub const ADMIN_TOKEN_HEADER: &str = "X-Admin-Token";

/// A request that carried the admin token.
pub struct Admin;

impl FromRequestParts<AppState> for Admin {
    type Rejection = AdminAuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let expected = state
            .admin_api_token
            .as_deref()
            .ok_or(AdminAuthError::Disabled)?;
        let given = parts
            .headers
            .get(ADMIN_TOKEN_HEADER)
            .ok_or(AdminAuthError::MissingToken)?;

        if bool::from(given.as_bytes().ct_eq(expected.as_bytes())) {
            Ok(Admin)
        } else {
            Err(AdminAuthError::InvalidToken)
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AdminAuthError {
    #[error("The admin API is disabled")]
    Disabled,
    #[error("Missing X-Admin-Token header")]
    MissingToken,
    #[error("Invalid admin token")]
    InvalidToken,
}

impl IntoResponse for AdminAuthError {
    fn into_response(self) -> Response {
        let status = match self {
            AdminAuthError::Disabled => StatusCode::SERVICE_UNAVAILABLE,
            AdminAuthError::MissingToken | AdminAuthError::InvalidToken => StatusCode::UNAUTHORIZED,
        };
        (
            status,
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}
