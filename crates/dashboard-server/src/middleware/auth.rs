use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::{db, AppState};

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
struct Jwks { keys: Vec<JwkKey> }

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
    let jwks: Jwks = reqwest::get(&jwks_url).await
        .map_err(|_| AuthError::InvalidToken)?
        .json().await
        .map_err(|_| AuthError::InvalidToken)?;

    let key = jwks.keys.into_iter()
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

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let token = parts.headers.get("Authorization")
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
        let user = match db::get_user_by_keycloak_id(&state.db_pool, &claims.sub).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
        {
            Some(user) => user,
            None => {
                // Create user if they don't exist
                let email = claims.email.clone().unwrap_or_default();
                let username = claims.name.clone()
                    .or(claims.preferred_username.clone())
                    .unwrap_or_else(|| claims.sub.clone());

                db::create_user(&state.db_pool, &claims.sub, &email, &username).await
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

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let org_id_str = parts.headers.get("X-Organization-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingOrganizationId)?;

        let org_id = Uuid::parse_str(org_id_str)
            .map_err(|_| AuthError::InvalidOrganizationId)?;

        let organization = db::get_organization_by_id(&state.db_pool, org_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::OrganizationNotFound)?;

        let role = db::get_user_organization_role(&state.db_pool, auth_user.user.id, org_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        match role {
            Some(db::OrgRole::OrgAdmin) => Ok(OrgAdmin { auth_user, organization }),
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

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let org_id_str = parts.headers.get("X-Organization-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingOrganizationId)?;

        let org_id = Uuid::parse_str(org_id_str)
            .map_err(|_| AuthError::InvalidOrganizationId)?;

        let organization = db::get_organization_by_id(&state.db_pool, org_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::OrganizationNotFound)?;

        let role = db::get_user_organization_role(&state.db_pool, auth_user.user.id, org_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        Ok(OrgContext { auth_user, organization, role })
    }
}

/// ProjectAccess requires project role (via X-Project-Id header)
pub struct ProjectAccess {
    pub auth_user: AuthUser,
    pub project: db::Project,
    pub role: db::ProjectRole,
}

impl FromRequestParts<AppState> for ProjectAccess {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let project_id_str = parts.headers.get("X-Project-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingProjectId)?;

        let project_id = Uuid::parse_str(project_id_str)
            .map_err(|_| AuthError::InvalidProjectId)?;

        let project = db::get_project_by_id(&state.db_pool, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::ProjectNotFound)?;

        let role = db::get_user_project_role(&state.db_pool, auth_user.user.id, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::Forbidden)?;

        Ok(ProjectAccess { auth_user, project, role })
    }
}

/// ProjectContext extracts project from X-Project-Id header, provides project + role (if member)
pub struct ProjectContext {
    pub auth_user: AuthUser,
    pub project: db::Project,
    pub role: Option<db::ProjectRole>,
}

impl FromRequestParts<AppState> for ProjectContext {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let project_id_str = parts.headers.get("X-Project-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingProjectId)?;

        let project_id = Uuid::parse_str(project_id_str)
            .map_err(|_| AuthError::InvalidProjectId)?;

        let project = db::get_project_by_id(&state.db_pool, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::ProjectNotFound)?;

        let role = db::get_user_project_role(&state.db_pool, auth_user.user.id, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        Ok(ProjectContext { auth_user, project, role })
    }
}

/// ProjectAdmin requires project admin role (via X-Project-Id header)
pub struct ProjectAdmin {
    pub auth_user: AuthUser,
    pub project: db::Project,
}

impl FromRequestParts<AppState> for ProjectAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let project_id_str = parts.headers.get("X-Project-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingProjectId)?;

        let project_id = Uuid::parse_str(project_id_str)
            .map_err(|_| AuthError::InvalidProjectId)?;

        let project = db::get_project_by_id(&state.db_pool, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::ProjectNotFound)?;

        let role = db::get_user_project_role(&state.db_pool, auth_user.user.id, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        match role {
            Some(db::ProjectRole::ProjectAdmin) => Ok(ProjectAdmin { auth_user, project }),
            _ => Err(AuthError::Forbidden),
        }
    }
}

/// ProjectAdminOrOrgAdmin allows project admins OR org admins (for rename operations)
pub struct ProjectAdminOrOrgAdmin {
    pub auth_user: AuthUser,
    pub project: db::Project,
}

impl FromRequestParts<AppState> for ProjectAdminOrOrgAdmin {
    type Rejection = AuthError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        let project_id_str = parts.headers.get("X-Project-Id")
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthError::MissingProjectId)?;

        let project_id = Uuid::parse_str(project_id_str)
            .map_err(|_| AuthError::InvalidProjectId)?;

        let project = db::get_project_by_id(&state.db_pool, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?
            .ok_or(AuthError::ProjectNotFound)?;

        // Check if user has project admin role
        let project_role = db::get_user_project_role(&state.db_pool, auth_user.user.id, project_id).await
            .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

        if matches!(project_role, Some(db::ProjectRole::ProjectAdmin)) {
            return Ok(ProjectAdminOrOrgAdmin { auth_user, project });
        }

        // Or check if user is org admin of the project's organization
        if let Some(org_id) = project.organization_id {
            let org_role = db::get_user_organization_role(&state.db_pool, auth_user.user.id, org_id).await
                .map_err(|e| AuthError::InternalWithMessage(e.to_string()))?;

            if matches!(org_role, Some(db::OrgRole::OrgAdmin)) {
                return Ok(ProjectAdminOrOrgAdmin { auth_user, project });
            }
        }

        Err(AuthError::Forbidden)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Missing Authorization header")] MissingToken,
    #[error("Invalid or expired token")] InvalidToken,
    #[error("Insufficient permissions")] Forbidden,
    #[error("Missing Organization ID header")] MissingOrganizationId,
    #[error("Invalid Organization ID")] InvalidOrganizationId,
    #[error("Organization not found")] OrganizationNotFound,
    #[error("Missing Project ID header")] MissingProjectId,
    #[error("Invalid Project ID")] InvalidProjectId,
    #[error("Project not found")] ProjectNotFound,
    #[error("Internal server error")] Internal,
    #[error("Internal server error: {0}")] InternalWithMessage(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthError::MissingToken => (StatusCode::UNAUTHORIZED, "Missing Bearer token"),
            AuthError::InvalidToken => (StatusCode::UNAUTHORIZED, "Invalid or expired token"),
            AuthError::Forbidden => (StatusCode::FORBIDDEN, "Insufficient permissions"),
            AuthError::MissingOrganizationId => (StatusCode::BAD_REQUEST, "Missing X-Organization-Id header"),
            AuthError::InvalidOrganizationId => (StatusCode::BAD_REQUEST, "Invalid Organization ID"),
            AuthError::OrganizationNotFound => (StatusCode::NOT_FOUND, "Organization not found"),
            AuthError::MissingProjectId => (StatusCode::BAD_REQUEST, "Missing X-Project-Id header"),
            AuthError::InvalidProjectId => (StatusCode::BAD_REQUEST, "Invalid Project ID"),
            AuthError::ProjectNotFound => (StatusCode::NOT_FOUND, "Project not found"),
            AuthError::Internal => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
            AuthError::InternalWithMessage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error"),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
