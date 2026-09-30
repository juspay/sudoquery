use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db::{self, Invitation, InvitationStatus, InvitationType, OrgRole, ProjectRole},
    middleware::{AuthUser, OrgAdmin, OrgContext, ProjectAccess},
    AppState,
};

// ============ Create Invitation ============

#[derive(Deserialize)]
pub struct CreateInvitationRequest {
    pub email: String,
    pub role: String, // "org_admin", "org_user", "project_admin", "project_user"
}

#[derive(Serialize)]
pub struct InvitationResponse {
    pub id: String,
    pub email: String,
    pub invitation_type: String,
    pub target_id: String,
    pub role: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
}

impl From<Invitation> for InvitationResponse {
    fn from(inv: Invitation) -> Self {
        InvitationResponse {
            id: inv.id.to_string(),
            email: inv.email,
            invitation_type: match inv.invitation_type {
                InvitationType::Organization => "organization".to_string(),
                InvitationType::Project => "project".to_string(),
            },
            target_id: inv.target_id.to_string(),
            role: inv.role,
            status: match inv.status {
                InvitationStatus::Pending => "pending".to_string(),
                InvitationStatus::Accepted => "accepted".to_string(),
                InvitationStatus::Revoked => "revoked".to_string(),
                InvitationStatus::Expired => "expired".to_string(),
            },
            created_at: inv.created_at.to_rfc3339(),
            expires_at: inv.expires_at.to_rfc3339(),
        }
    }
}

// ============ Create Organization Invitation ============

pub async fn create_organization_invitation(
    State(state): State<AppState>,
    OrgAdmin { organization, auth_user }: OrgAdmin,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<InvitationResponse>), InvitationError> {
    // Prevent self-invite
    if req.email == auth_user.user.email {
        return Err(InvitationError::CannotInviteSelf);
    }

    // Validate role for organization
    let role = match req.role.as_str() {
        "org_admin" => "org_admin",
        "org_user" => "org_user",
        _ => return Err(InvitationError::InvalidRole),
    };

    let invitation = db::create_invitation(
        &state.db_pool,
        &req.email,
        InvitationType::Organization,
        organization.id,
        role,
        auth_user.user.id,
    )
    .await
    .map_err(|e| InvitationError::Database(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(invitation.into())))
}

// ============ Create Project Invitation ============

pub async fn create_project_invitation(
    State(state): State<AppState>,
    ProjectAccess { auth_user, project, .. }: ProjectAccess,
    Json(req): Json<CreateInvitationRequest>,
) -> Result<(StatusCode, Json<InvitationResponse>), InvitationError> {
    // Prevent self-invite
    if req.email == auth_user.user.email {
        return Err(InvitationError::CannotInviteSelf);
    }

    // Validate role for project
    let role = match req.role.as_str() {
        "project_admin" => "project_admin",
        "project_user" => "project_user",
        _ => return Err(InvitationError::InvalidRole),
    };

    let invitation = db::create_invitation(
        &state.db_pool,
        &req.email,
        InvitationType::Project,
        project.id,
        role,
        auth_user.user.id,
    )
    .await
    .map_err(|e| InvitationError::Database(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(invitation.into())))
}

// ============ List My Pending Invitations ============

pub async fn list_my_invitations(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<InvitationResponse>>, InvitationError> {
    let invitations = db::list_invitations_for_user(&state.db_pool, &user.email)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?;

    let response: Vec<InvitationResponse> = invitations.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

// ============ List Invitations I Sent ============

pub async fn list_sent_invitations(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<InvitationResponse>>, InvitationError> {
    let invitations = db::list_invitations_by_inviter(&state.db_pool, user.id)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?;

    let response: Vec<InvitationResponse> = invitations.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

// ============ List Organization Invitations ============

pub async fn list_organization_invitations(
    State(state): State<AppState>,
    OrgContext { organization, .. }: OrgContext,
) -> Result<Json<Vec<InvitationResponse>>, InvitationError> {
    let invitations = db::list_invitations_for_target(
        &state.db_pool,
        organization.id,
        InvitationType::Organization,
    )
    .await
    .map_err(|e| InvitationError::Database(e.to_string()))?;

    let response: Vec<InvitationResponse> = invitations.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

// ============ List Project Invitations ============

pub async fn list_project_invitations(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> Result<Json<Vec<InvitationResponse>>, InvitationError> {
    let invitations = db::list_invitations_for_target(
        &state.db_pool,
        project.id,
        InvitationType::Project,
    )
    .await
    .map_err(|e| InvitationError::Database(e.to_string()))?;

    let response: Vec<InvitationResponse> = invitations.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

// ============ Accept Invitation ============

#[derive(Serialize)]
pub struct AcceptInvitationResponse {
    pub message: String,
    pub invitation_id: String,
}

pub async fn accept_invitation(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
    Path(invitation_id): Path<String>,
) -> Result<(StatusCode, Json<AcceptInvitationResponse>), InvitationError> {
    let invitation_uuid = Uuid::parse_str(&invitation_id)
        .map_err(|_| InvitationError::InvalidInvitationId)?;

    // Get invitation details first
    let invitation = db::get_invitation_by_id(&state.db_pool, invitation_uuid)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?
        .ok_or(InvitationError::NotFound)?;

    // Verify the invitation is for this user
    if invitation.email != user.email {
        return Err(InvitationError::Forbidden);
    }

    // Check if already processed
    match invitation.status {
        InvitationStatus::Accepted => return Err(InvitationError::AlreadyAccepted),
        InvitationStatus::Revoked => return Err(InvitationError::Revoked),
        InvitationStatus::Expired => return Err(InvitationError::Expired),
        InvitationStatus::Pending => {}
    }

    // Accept the invitation
    let invitation = db::accept_invitation(&state.db_pool, invitation_uuid)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?;

    // Add user to organization or project based on invitation type
    match invitation.invitation_type {
        InvitationType::Organization => {
            let role = match invitation.role.as_str() {
                "org_admin" => OrgRole::OrgAdmin,
                _ => OrgRole::OrgUser,
            };
            db::add_user_to_organization(&state.db_pool, user.id, invitation.target_id, &role)
                .await
                .map_err(|e| InvitationError::Database(e.to_string()))?;
        }
        InvitationType::Project => {
            let role = match invitation.role.as_str() {
                "project_admin" => ProjectRole::ProjectAdmin,
                _ => ProjectRole::ProjectUser,
            };
            db::add_user_to_project(&state.db_pool, user.id, invitation.target_id, &role)
                .await
                .map_err(|e| InvitationError::Database(e.to_string()))?;
        }
    }

    Ok((StatusCode::OK, Json(AcceptInvitationResponse {
        message: "Invitation accepted successfully".to_string(),
        invitation_id: invitation.id.to_string(),
    })))
}

// ============ Revoke Invitation ============

#[derive(Serialize)]
pub struct RevokeInvitationResponse {
    pub message: String,
}

pub async fn revoke_invitation(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
    Path(invitation_id): Path<String>,
) -> Result<(StatusCode, Json<RevokeInvitationResponse>), InvitationError> {
    let invitation_uuid = Uuid::parse_str(&invitation_id)
        .map_err(|_| InvitationError::InvalidInvitationId)?;

    // Get invitation details
    let invitation = db::get_invitation_by_id(&state.db_pool, invitation_uuid)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?
        .ok_or(InvitationError::NotFound)?;

    // Only the inviter or an admin can revoke
    // For simplicity, we'll check if the user is the inviter
    // In a real app, you'd also check if user is org/project admin
    if invitation.invited_by != user.id {
        return Err(InvitationError::Forbidden);
    }

    // Check if already processed
    match invitation.status {
        InvitationStatus::Accepted => return Err(InvitationError::AlreadyAccepted),
        InvitationStatus::Revoked => return Err(InvitationError::AlreadyRevoked),
        InvitationStatus::Expired => return Err(InvitationError::Expired),
        InvitationStatus::Pending => {}
    }

    db::revoke_invitation(&state.db_pool, invitation_uuid)
        .await
        .map_err(|e| InvitationError::Database(e.to_string()))?;

    Ok((StatusCode::OK, Json(RevokeInvitationResponse {
        message: "Invitation revoked successfully".to_string(),
    })))
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum InvitationError {
    #[error("Invalid role")] InvalidRole,
    #[error("Cannot invite yourself")] CannotInviteSelf,
    #[error("Invalid invitation ID")] InvalidInvitationId,
    #[error("Invitation not found")] NotFound,
    #[error("Invitation already accepted")] AlreadyAccepted,
    #[error("Invitation already revoked")] AlreadyRevoked,
    #[error("Invitation expired")] Expired,
    #[error("Invitation revoked")] Revoked,
    #[error("Forbidden")] Forbidden,
    #[error("Database error: {0}")] Database(String),
}

impl axum::response::IntoResponse for InvitationError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message): (StatusCode, String) = match self {
            InvitationError::InvalidRole => (StatusCode::BAD_REQUEST, "Invalid role".to_string()),
            InvitationError::CannotInviteSelf => (StatusCode::BAD_REQUEST, "Cannot invite yourself".to_string()),
            InvitationError::InvalidInvitationId => (StatusCode::BAD_REQUEST, "Invalid invitation ID".to_string()),
            InvitationError::NotFound => (StatusCode::NOT_FOUND, "Invitation not found".to_string()),
            InvitationError::AlreadyAccepted => (StatusCode::CONFLICT, "Invitation already accepted".to_string()),
            InvitationError::AlreadyRevoked => (StatusCode::CONFLICT, "Invitation already revoked".to_string()),
            InvitationError::Expired => (StatusCode::GONE, "Invitation expired".to_string()),
            InvitationError::Revoked => (StatusCode::GONE, "Invitation revoked".to_string()),
            InvitationError::Forbidden => (StatusCode::FORBIDDEN, "You do not have permission to perform this action".to_string()),
            InvitationError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
