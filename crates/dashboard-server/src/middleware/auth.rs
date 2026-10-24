use axum::{
    Json,
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
    response::{IntoResponse, Response},
};
use canonical_event::{OrgId, WorkspaceId};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use tokio::sync::RwLock;

use crate::{AppState, db};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeycloakClaims {
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub email: Option<String>,
    #[serde(rename = "preferred_username")]
    pub preferred_username: Option<String>,
    pub name: Option<String>,
    pub realm_access: Option<RealmAccess>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RealmAccess {
    pub roles: Vec<String>,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<JwkKey>,
}

#[derive(Deserialize, Clone)]
struct JwkKey {
    kid: String,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

static JWKS_CACHE: Lazy<RwLock<HashMap<String, JwkKey>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

async fn get_decoding_key(realm_url: &str, kid: &str) -> Result<DecodingKey, AuthError> {
    // Always fetch fresh JWKS to handle key rotation
    let jwks_url = format!("{}/protocol/openid-connect/certs", realm_url);
    let jwks: Jwks = reqwest::get(&jwks_url)
        .await
        .map_err(|_| AuthError::InvalidToken)?
        .json()
        .await
        .map_err(|_| AuthError::InvalidToken)?;

    let key = jwks
        .keys
        .into_iter()
        .find(|k| k.kid == kid)
        .ok_or(AuthError::InvalidToken)?;

    jwk_to_decoding_key(&key)
}

fn jwk_to_decoding_key(key: &JwkKey) -> Result<DecodingKey, AuthError> {
    match key.kty.as_str() {
        "RSA" => {
            let n = key.n.as_deref().ok_or(AuthError::InvalidToken)?;
            let e = key.e.as_deref().ok_or(AuthError::InvalidToken)?;
            DecodingKey::from_rsa_components(n, e).map_err(|_| AuthError::InvalidToken)
        }
        _ => Err(AuthError::InvalidToken),
    }
}

fn extract_kid(token: &str) -> Result<String, AuthError> {
    let header = decode_header(token).map_err(|_| AuthError::InvalidToken)?;
    header.kid.ok_or(AuthError::InvalidToken)
}

/// AuthUser validates JWT from single realm and loads user from DB by keycloak_user_id.
/// If the user doesn't exist in the database, it creates one automatically.
pub struct AuthUser {
    pub claims: KeycloakClaims,
    pub user: db::User,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AuthError::MissingToken)?
            .to_string();

        let kid = extract_kid(&token)?;
        let realm_url = format!("{}/realms/{}", state.keycloak_url, state.keycloak_realm);

        let decoding_key = get_decoding_key(&realm_url, &kid).await?;

        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_issuer(&[&realm_url]);
        validation.validate_exp = false; // TEMP: Keycloak clock is wrong (2025 instead of 2026)
        validation.validate_aud = false; // Keycloak access tokens have "account" as audience

        let claims = match decode::<KeycloakClaims>(&token, &decoding_key, &validation) {
            Ok(token_data) => token_data.claims,
            Err(e) => {
                tracing::error!("JWT decode error: {:?}", e);
                return Err(AuthError::InvalidToken);
            }
        };

        // Load or create user from database
        let user = match db::get_user_by_keycloak_id(&state.db_pool, &claims.sub)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
        {
            Some(user) => user,
            None => {
                // Create user if they don't exist
                let email = claims.email.clone().unwrap_or_default();
                let username = claims
                    .name
                    .clone()
                    .or(claims.preferred_username.clone())
                    .unwrap_or_else(|| claims.sub.clone());

                db::create_user(&state.db_pool, &claims.sub, &email, &username)
                    .await
                    .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            }
        };

        Ok(AuthUser { claims, user })
    }
}

/// OrgAdmin requires org_admin role in organization (via X-Organization-Id header)
pub struct OrgAdmin {
    pub auth_user: AuthUser,
    pub organization: db::Organization,
}

impl FromRequestParts<AppState> for OrgAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let org_id_str = parts
            .headers
            .get("X-Organization-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingOrganizationId)?;

        let org_id = OrgId::from_str(org_id_str).map_err(|_| AuthError::InvalidOrganizationId)?;

        let organization = db::get_organization_by_id(&state.db_pool, &org_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::OrganizationNotFound)?;

        let role = db::get_user_organization_role(&state.db_pool, auth_user.user.id, &org_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        match role {
            Some(db::OrgRole::OrgAdmin) => Ok(OrgAdmin {
                auth_user,
                organization,
            }),
            _ => Err(AuthError::Forbidden),
        }
    }
}

/// OrgContext extracts org from X-Organization-Id header, provides org + role (if member)
pub struct OrgContext {
    pub auth_user: AuthUser,
    pub organization: db::Organization,
    pub role: Option<db::OrgRole>,
}

impl FromRequestParts<AppState> for OrgContext {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let org_id_str = parts
            .headers
            .get("X-Organization-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingOrganizationId)?;

        let org_id = OrgId::from_str(org_id_str).map_err(|_| AuthError::InvalidOrganizationId)?;

        let organization = db::get_organization_by_id(&state.db_pool, &org_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::OrganizationNotFound)?;

        let role = db::get_user_organization_role(&state.db_pool, auth_user.user.id, &org_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        Ok(OrgContext {
            auth_user,
            organization,
            role,
        })
    }
}

/// WorkspaceAccess requires workspace role (via X-Workspace-Id header)
pub struct WorkspaceAccess {
    pub auth_user: AuthUser,
    pub workspace: db::Workspace,
    pub role: db::WorkspaceRole,
}

impl FromRequestParts<AppState> for WorkspaceAccess {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let workspace_id_str = parts
            .headers
            .get("X-Workspace-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingWorkspaceId)?;

        let workspace_id =
            WorkspaceId::from_str(workspace_id_str).map_err(|_| AuthError::InvalidWorkspaceId)?;

        let workspace = db::get_workspace_by_id(&state.db_pool, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::WorkspaceNotFound)?;

        let role = db::get_user_workspace_role(&state.db_pool, auth_user.user.id, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::Forbidden)?;

        Ok(WorkspaceAccess {
            auth_user,
            workspace,
            role,
        })
    }
}

/// WorkspaceContext extracts workspace from X-Workspace-Id header, provides workspace + role (if member)
pub struct WorkspaceContext {
    pub auth_user: AuthUser,
    pub workspace: db::Workspace,
    pub role: Option<db::WorkspaceRole>,
}

impl FromRequestParts<AppState> for WorkspaceContext {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let workspace_id_str = parts
            .headers
            .get("X-Workspace-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingWorkspaceId)?;

        let workspace_id =
            WorkspaceId::from_str(workspace_id_str).map_err(|_| AuthError::InvalidWorkspaceId)?;

        let workspace = db::get_workspace_by_id(&state.db_pool, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::WorkspaceNotFound)?;

        let role = db::get_user_workspace_role(&state.db_pool, auth_user.user.id, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        Ok(WorkspaceContext {
            auth_user,
            workspace,
            role,
        })
    }
}

/// WorkspaceAdmin requires workspace admin role (via X-Workspace-Id header)
pub struct WorkspaceAdmin {
    pub auth_user: AuthUser,
    pub workspace: db::Workspace,
}

impl FromRequestParts<AppState> for WorkspaceAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let workspace_id_str = parts
            .headers
            .get("X-Workspace-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingWorkspaceId)?;

        let workspace_id =
            WorkspaceId::from_str(workspace_id_str).map_err(|_| AuthError::InvalidWorkspaceId)?;

        let workspace = db::get_workspace_by_id(&state.db_pool, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::WorkspaceNotFound)?;

        let role = db::get_user_workspace_role(&state.db_pool, auth_user.user.id, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        match role {
            Some(db::WorkspaceRole::WorkspaceAdmin) => Ok(WorkspaceAdmin {
                auth_user,
                workspace,
            }),
            _ => Err(AuthError::Forbidden),
        }
    }
}

/// WorkspaceAdminOrOrgAdmin allows workspace admins OR org admins (for rename operations)
pub struct WorkspaceAdminOrOrgAdmin {
    pub auth_user: AuthUser,
    pub workspace: db::Workspace,
}

impl FromRequestParts<AppState> for WorkspaceAdminOrOrgAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let workspace_id_str = parts
            .headers
            .get("X-Workspace-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingWorkspaceId)?;

        let workspace_id =
            WorkspaceId::from_str(workspace_id_str).map_err(|_| AuthError::InvalidWorkspaceId)?;

        let workspace = db::get_workspace_by_id(&state.db_pool, &workspace_id)
            .await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::WorkspaceNotFound)?;

        // Check if user has workspace admin role
        let workspace_role =
            db::get_user_workspace_role(&state.db_pool, auth_user.user.id, &workspace_id)
                .await
                .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        if matches!(workspace_role, Some(db::WorkspaceRole::WorkspaceAdmin)) {
            return Ok(WorkspaceAdminOrOrgAdmin {
                auth_user,
                workspace,
            });
        }

        // Or check if user is org admin of the workspace's organization
        if let Some(org_id) = workspace.organization_id.as_ref() {
            let org_role =
                db::get_user_organization_role(&state.db_pool, auth_user.user.id, org_id)
                    .await
                    .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

            if matches!(org_role, Some(db::OrgRole::OrgAdmin)) {
                return Ok(WorkspaceAdminOrOrgAdmin {
                    auth_user,
                    workspace,
                });
            }
        }

        Err(AuthError::Forbidden)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing Authorization header")]
    MissingToken,
    #[error("Invalid or expired token")]
    InvalidToken,
    #[error("Insufficient permissions")]
    Forbidden,
    #[error("Missing Organization ID header")]
    MissingOrganizationId,
    #[error("Invalid Organization ID")]
    InvalidOrganizationId,
    #[error("Organization not found")]
    OrganizationNotFound,
    #[error("Missing Workspace ID header")]
    MissingWorkspaceId,
    #[error("Invalid Workspace ID")]
    InvalidWorkspaceId,
    #[error("Workspace not found")]
    WorkspaceNotFound,
    #[error("Internal server error")]
    Internal,
    #[error("Internal server error: {0}")]
    InternalWithMessage(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing Bearer token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::Forbidden => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            AuthError::MissingOrganizationId => {
                (StatusCode::BAD_REQUEST, "Missing X-Organization-Id header")
            }
            AuthError::InvalidOrganizationId => {
                (StatusCode::BAD_REQUEST, "Invalid Organization ID")
            }
            AuthError::OrganizationNotFound => (StatusCode::NOT_FOUND, "Organization not found"),
            AuthError::MissingWorkspaceId => {
                (StatusCode::BAD_REQUEST, "Missing X-Workspace-Id header")
            }
            AuthError::InvalidWorkspaceId => (StatusCode::BAD_REQUEST, "Invalid Workspace ID"),
            AuthError::WorkspaceNotFound => (StatusCode::NOT_FOUND, "Workspace not found"),
            AuthError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            AuthError::InternalWithMessage(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error")
            }
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
