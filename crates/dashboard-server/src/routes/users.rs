use axum::{extract::State, http::StatusCode, response::Json};
use canonical_event::ProjectId;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    AppState, db,
    keycloak::KeycloakError,
    middleware::{AuthError, AuthUser, OrgAdmin, OrgContext},
};

// ============ Create User (Organization Admin) ============

#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct CreateUserResponse {
    pub user_id: String,
    pub keycloak_user_id: String,
    pub username: String,
    pub email: String,
}

pub async fn create_user(
    State(state): State<AppState>,
    OrgAdmin {
        organization: _, ..
    }: OrgAdmin,
    Json(req): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<CreateUserResponse>), ApiError> {
    // Create user in Keycloak
    let keycloak_user_id = state
        .keycloak
        .create_user(&req.username, &req.email)
        .await
        .map_err(ApiError::Keycloak)?;

    // Set temporary password
    state
        .keycloak
        .set_password(&keycloak_user_id, &state.temp_password, false)
        .await
        .map_err(ApiError::Keycloak)?;

    // Create user in database
    let user = db::create_user(&state.db_pool, &keycloak_user_id, &req.email, &req.username)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(CreateUserResponse {
            user_id: user.id.to_string(),
            keycloak_user_id: user.keycloak_user_id,
            username: user.username,
            email: user.email,
        }),
    ))
}

// ============ Delete User (Organization Admin) ============

pub async fn delete_user(
    State(state): State<AppState>,
    OrgAdmin {
        auth_user,
        organization,
    }: OrgAdmin,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| ApiError::InvalidUserId)?;

    // Prevent users from deleting themselves
    if user_uuid == auth_user.user.id {
        return Err(ApiError::CannotRemoveSelf);
    }

    // Get user from database
    let user = db::get_user_by_id(&state.db_pool, user_uuid)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or(ApiError::UserNotFound)?;

    // Check if user is a member of the organization
    let membership = db::get_organization_membership(&state.db_pool, user_uuid, &organization.id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    if membership.is_none() {
        return Err(ApiError::UserNotFound);
    }

    // Delete user from Keycloak
    state
        .keycloak
        .delete_user(&user.keycloak_user_id)
        .await
        .map_err(ApiError::Keycloak)?;

    // Soft delete user from database
    db::soft_delete_user(&state.db_pool, user_uuid)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Get Current User Info ============

#[derive(Serialize)]
pub struct MeResponse {
    pub user_id: String,
    pub keycloak_user_id: String,
    pub email: String,
    pub username: String,
}

pub async fn me(AuthUser { user, .. }: AuthUser) -> Json<MeResponse> {
    Json(MeResponse {
        user_id: user.id.to_string(),
        keycloak_user_id: user.keycloak_user_id,
        email: user.email,
        username: user.username,
    })
}

// ============ Add User to Project ============

#[derive(Deserialize)]
pub struct AddProjectMemberRequest {
    pub user_id: Option<String>,
    pub email: Option<String>,
    pub role: String,
}

#[derive(Serialize)]
pub struct ProjectMemberResponse {
    pub user_id: String,
    pub project_id: String,
    pub role: String,
}

pub async fn add_project_member(
    State(state): State<AppState>,
    OrgContext {
        auth_user,
        organization: _,
        role: org_role,
    }: OrgContext,
    headers: axum::http::HeaderMap,
    Json(req): Json<AddProjectMemberRequest>,
) -> Result<(StatusCode, Json<ProjectMemberResponse>), ApiError> {
    let project_id_str = headers
        .get("X-Project-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::InvalidProjectId)?;

    let project_id = ProjectId::from_str(project_id_str).map_err(|_| ApiError::InvalidProjectId)?;

    // Resolve user by ID or email
    let user_uuid = if let Some(user_id) = req.user_id {
        Uuid::parse_str(&user_id).map_err(|_| ApiError::InvalidUserId)?
    } else if let Some(email) = req.email {
        let user = db::get_user_by_email(&state.db_pool, &email)
            .await
            .map_err(|e| ApiError::Database(e.to_string()))?
            .ok_or(ApiError::UserNotFound)?;
        user.id
    } else {
        return Err(ApiError::InvalidUserId);
    };

    // Verify project exists and belongs to the organization context
    let _project = db::get_project_by_id(&state.db_pool, &project_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or(ApiError::ProjectNotFound)?;

    // Check if the requesting user is project admin or org admin
    match org_role {
        Some(db::OrgRole::OrgAdmin) => {} // Org admin can add anyone
        Some(db::OrgRole::OrgUser) | None => {
            // Check if requesting user is project admin
            let pm = db::get_project_membership(&state.db_pool, auth_user.user.id, &project_id)
                .await
                .map_err(|e| ApiError::Database(e.to_string()))?;

            match pm {
                Some(m) if m.role == db::ProjectRole::ProjectAdmin => {}
                _ => return Err(ApiError::Forbidden),
            }
        }
    };

    let role = match req.role.as_str() {
        "project_admin" => db::ProjectRole::ProjectAdmin,
        "project_user" => db::ProjectRole::ProjectUser,
        _ => return Err(ApiError::InvalidRole),
    };

    let membership = db::add_user_to_project(&state.db_pool, user_uuid, &project_id, &role)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(ProjectMemberResponse {
            user_id: membership.user_id.to_string(),
            project_id: membership.project_id.to_string(),
            role: membership.role.as_str().to_string(),
        }),
    ))
}

// ============ Remove User from Project ============

pub async fn remove_project_member(
    State(state): State<AppState>,
    OrgContext {
        auth_user,
        organization: _,
        role: org_role,
    }: OrgContext,
    headers: axum::http::HeaderMap,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<StatusCode, ApiError> {
    let project_id_str = headers
        .get("X-Project-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::InvalidProjectId)?;

    let project_id = ProjectId::from_str(project_id_str).map_err(|_| ApiError::InvalidProjectId)?;

    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| ApiError::InvalidUserId)?;

    // Prevent users from removing themselves
    if user_uuid == auth_user.user.id {
        return Err(ApiError::CannotRemoveSelf);
    }

    // Verify project exists
    let _project = db::get_project_by_id(&state.db_pool, &project_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?
        .ok_or(ApiError::ProjectNotFound)?;

    // Check if user is project admin or org admin
    match org_role {
        Some(db::OrgRole::OrgAdmin) => {} // Org admin can remove anyone
        Some(db::OrgRole::OrgUser) | None => {
            return Err(ApiError::Forbidden);
        }
    };

    db::remove_user_from_project(&state.db_pool, user_uuid, &project_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ List Project Members ============

#[derive(Serialize)]
pub struct ProjectMemberListResponse {
    pub user_id: String,
    pub email: String,
    pub username: String,
    pub role: String,
}

pub async fn list_project_members(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<Vec<ProjectMemberListResponse>>, ApiError> {
    let project_id_str = headers
        .get("X-Project-Id")
        .and_then(|v| v.to_str().ok())
        .ok_or(ApiError::InvalidProjectId)?;

    let project_id = ProjectId::from_str(project_id_str).map_err(|_| ApiError::InvalidProjectId)?;

    let members = db::list_project_members(&state.db_pool, &project_id)
        .await
        .map_err(|e| ApiError::Database(e.to_string()))?;

    let response: Vec<ProjectMemberListResponse> = members
        .into_iter()
        .map(|(id, email, username, role)| ProjectMemberListResponse {
            user_id: id.to_string(),
            email,
            username,
            role: role.as_str().to_string(),
        })
        .collect();

    Ok(Json(response))
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Keycloak(#[from] KeycloakError),
    #[error("Invalid user ID")]
    InvalidUserId,
    #[error("Invalid project ID")]
    InvalidProjectId,
    #[error("Invalid role")]
    InvalidRole,
    #[error("User not found")]
    UserNotFound,
    #[error("Project not found")]
    ProjectNotFound,
    #[error("Forbidden")]
    Forbidden,
    #[error("Cannot remove yourself")]
    CannotRemoveSelf,
    #[error("Database error: {0}")]
    Database(String),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message): (StatusCode, String) = match self {
            ApiError::Auth(e) => return e.into_response(),
            ApiError::Keycloak(e) => return e.into_response(),
            ApiError::InvalidUserId => (StatusCode::BAD_REQUEST, "Invalid user ID".to_string()),
            ApiError::InvalidProjectId => {
                (StatusCode::BAD_REQUEST, "Invalid project ID".to_string())
            }
            ApiError::InvalidRole => (StatusCode::BAD_REQUEST, "Invalid role".to_string()),
            ApiError::UserNotFound => (StatusCode::NOT_FOUND, "User not found".to_string()),
            ApiError::ProjectNotFound => (StatusCode::NOT_FOUND, "Project not found".to_string()),
            ApiError::Forbidden => (StatusCode::FORBIDDEN, "Forbidden".to_string()),
            ApiError::CannotRemoveSelf => (
                StatusCode::BAD_REQUEST,
                "Cannot remove yourself".to_string(),
            ),
            ApiError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
