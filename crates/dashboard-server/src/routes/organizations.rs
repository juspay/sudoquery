use axum::{extract::State, http::StatusCode, response::Json};
use sea_orm::{ActiveModelTrait, Set};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    db::{self, OrgRole},
    entities,
    middleware::{AuthError, AuthUser, OrgAdmin, OrgContext},
};

// ============ Create Organization ============

#[derive(Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
}

#[derive(Serialize)]
pub struct OrganizationResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
}

pub async fn create_organization(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
    Json(req): Json<CreateOrganizationRequest>,
) -> Result<(StatusCode, Json<OrganizationResponse>), OrganizationError> {
    let org = db::create_organization(&state.db_pool, &req.name)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    // Add creator as org_admin
    db::add_user_to_organization(&state.db_pool, user.id, org.id, &OrgRole::OrgAdmin)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(OrganizationResponse {
            id: org.id.to_string(),
            name: org.name,
            created_at: org.created_at.to_rfc3339(),
        }),
    ))
}

// ============ Delete Organization ============

pub async fn delete_organization(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
) -> Result<StatusCode, OrganizationError> {
    db::soft_delete_organization(&state.db_pool, organization.id)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct MyOrganizationResponse {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub access_level: String,
}

pub async fn list_my_organizations(
    State(state): State<AppState>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<Vec<MyOrganizationResponse>>, OrganizationError> {
    let orgs = db::list_user_organizations(&state.db_pool, user.id)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    let response: Vec<MyOrganizationResponse> = orgs
        .into_iter()
        .map(|o| MyOrganizationResponse {
            id: o.organization.id.to_string(),
            name: o.organization.name,
            created_at: o.organization.created_at.to_rfc3339(),
            access_level: o.access_level,
        })
        .collect();

    Ok(Json(response))
}

// ============ Get Organization by ID ============

pub async fn get_organization(
    OrgContext { organization, .. }: OrgContext,
) -> Result<Json<OrganizationResponse>, OrganizationError> {
    Ok(Json(OrganizationResponse {
        id: organization.id.to_string(),
        name: organization.name,
        created_at: organization.created_at.to_rfc3339(),
    }))
}

// ============ Update Organization Name ============

#[derive(Deserialize)]
pub struct UpdateOrganizationRequest {
    pub name: String,
}

pub async fn update_organization(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Json(req): Json<UpdateOrganizationRequest>,
) -> Result<Json<OrganizationResponse>, OrganizationError> {
    use sea_orm::EntityTrait;

    // Fetch the SeaORM entity
    let org = entities::organization::Entity::find_by_id(organization.id)
        .one(&state.db_conn)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?
        .ok_or(OrganizationError::NotFound)?;

    let mut org_active: entities::organization::ActiveModel = org.into();
    org_active.name = Set(req.name);
    org_active.updated_at = Set(chrono::Utc::now().into());

    let updated = org_active
        .update(&state.db_conn)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok(Json(OrganizationResponse {
        id: updated.id.to_string(),
        name: updated.name,
        created_at: updated.created_at.to_rfc3339(),
    }))
}

// ============ Add User to Organization ============

#[derive(Deserialize)]
pub struct AddOrgMemberRequest {
    pub user_id: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct OrgMemberResponse {
    pub user_id: String,
    pub organization_id: String,
    pub role: String,
}

pub async fn add_organization_member(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    Json(req): Json<AddOrgMemberRequest>,
) -> Result<(StatusCode, Json<OrgMemberResponse>), OrganizationError> {
    let user_id = Uuid::parse_str(&req.user_id).map_err(|_| OrganizationError::InvalidUserId)?;

    let role = match req.role.as_str() {
        "org_admin" => OrgRole::OrgAdmin,
        "org_user" => OrgRole::OrgUser,
        _ => return Err(OrganizationError::InvalidRole),
    };

    let membership = db::add_user_to_organization(&state.db_pool, user_id, organization.id, &role)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(OrgMemberResponse {
            user_id: membership.user_id.to_string(),
            organization_id: membership.organization_id.to_string(),
            role: membership.role.as_str().to_string(),
        }),
    ))
}

// ============ Remove User from Organization ============

pub async fn remove_organization_member(
    State(state): State<AppState>,
    OrgAdmin {
        auth_user,
        organization,
    }: OrgAdmin,
    axum::extract::Path(user_id): axum::extract::Path<String>,
) -> Result<StatusCode, OrganizationError> {
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| OrganizationError::InvalidUserId)?;

    // Prevent users from removing themselves
    if user_uuid == auth_user.user.id {
        return Err(OrganizationError::CannotRemoveSelf);
    }

    db::remove_user_from_organization(&state.db_pool, user_uuid, organization.id)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ List Organization Members ============

#[derive(Serialize)]
pub struct OrgMemberListResponse {
    pub user_id: String,
    pub email: String,
    pub username: String,
    pub role: String,
}

pub async fn list_organization_members(
    State(state): State<AppState>,
    OrgContext { organization, .. }: OrgContext,
) -> Result<Json<Vec<OrgMemberListResponse>>, OrganizationError> {
    let members = db::list_organization_members(&state.db_pool, organization.id)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    let response: Vec<OrgMemberListResponse> = members
        .into_iter()
        .map(|(id, email, username, role)| OrgMemberListResponse {
            user_id: id.to_string(),
            email,
            username,
            role: role.as_str().to_string(),
        })
        .collect();

    Ok(Json(response))
}

// ============ Update Organization Member Role ============

#[derive(Deserialize)]
pub struct UpdateOrgMemberRoleRequest {
    pub role: String,
}

pub async fn update_organization_member_role(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    axum::extract::Path(user_id): axum::extract::Path<String>,
    Json(req): Json<UpdateOrgMemberRoleRequest>,
) -> Result<StatusCode, OrganizationError> {
    let user_uuid = Uuid::parse_str(&user_id).map_err(|_| OrganizationError::InvalidUserId)?;

    let role = match req.role.as_str() {
        "org_admin" => OrgRole::OrgAdmin,
        "org_user" => OrgRole::OrgUser,
        _ => return Err(OrganizationError::InvalidRole),
    };

    db::update_organization_membership_role(&state.db_pool, user_uuid, organization.id, &role)
        .await
        .map_err(|e| OrganizationError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Error Types ============

#[derive(Debug, thiserror::Error)]
pub enum OrganizationError {
    #[error("Invalid user ID")]
    InvalidUserId,
    #[error("Invalid role")]
    InvalidRole,
    #[error("Organization not found")]
    NotFound,
    #[error("Cannot remove yourself")]
    CannotRemoveSelf,
    #[error("Database error: {0}")]
    Database(String),
    #[error("Auth error: {0}")]
    Auth(#[from] AuthError),
}

impl axum::response::IntoResponse for OrganizationError {
    fn into_response(self) -> axum::response::Response {
        use axum::http::StatusCode;

        let (status, message): (StatusCode, String) = match self {
            OrganizationError::InvalidUserId => {
                (StatusCode::BAD_REQUEST, "Invalid user ID".to_string())
            }
            OrganizationError::InvalidRole => (
                StatusCode::BAD_REQUEST,
                "Invalid role. Must be 'org_admin' or 'org_user'".to_string(),
            ),
            OrganizationError::NotFound => {
                (StatusCode::NOT_FOUND, "Organization not found".to_string())
            }
            OrganizationError::CannotRemoveSelf => (
                StatusCode::BAD_REQUEST,
                "Cannot remove yourself".to_string(),
            ),
            OrganizationError::Database(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            OrganizationError::Auth(e) => return e.into_response(),
        };
        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
