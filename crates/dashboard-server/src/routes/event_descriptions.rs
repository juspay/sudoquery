use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use canonical_event::WorkspaceId;
use serde::{Deserialize, Serialize};

use crate::{
    AppState, db,
    middleware::{AuthUser, OrgAdmin, OrgContext, WorkspaceAdmin, WorkspaceContext},
};

// ============ Create/Update Event Description ============

#[derive(Deserialize)]
pub struct UpsertEventDescriptionRequest {
    pub event_name: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct EventDescriptionResponse {
    pub id: String,
    pub workspace_id: String,
    pub event_name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Create or update event description (requires workspace admin)
pub async fn upsert_event_description(
    State(state): State<AppState>,
    WorkspaceAdmin { workspace, .. }: WorkspaceAdmin,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Result<(StatusCode, Json<EventDescriptionResponse>), EventDescriptionError> {
    let desc = db::upsert_event_description(
        &state.db_pool,
        &workspace.id,
        &req.event_name,
        &req.description,
    )
    .await
    .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(EventDescriptionResponse {
            id: desc.id.to_string(),
            workspace_id: desc.workspace_id.to_string(),
            event_name: desc.event_name,
            description: desc.description,
            created_at: desc.created_at.to_rfc3339(),
            updated_at: desc.updated_at.to_rfc3339(),
        }),
    ))
}

/// Create or update event description as org admin (requires X-Workspace-Id header)
pub async fn upsert_event_description_as_org_admin(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    WorkspaceContext { workspace, .. }: WorkspaceContext,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Result<(StatusCode, Json<EventDescriptionResponse>), EventDescriptionError> {
    // Verify workspace belongs to this organization
    if workspace.organization_id != Some(organization.id) {
        return Err(EventDescriptionError::Forbidden);
    }

    let desc = db::upsert_event_description(
        &state.db_pool,
        &workspace.id,
        &req.event_name,
        &req.description,
    )
    .await
    .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(EventDescriptionResponse {
            id: desc.id.to_string(),
            workspace_id: desc.workspace_id.to_string(),
            event_name: desc.event_name,
            description: desc.description,
            created_at: desc.created_at.to_rfc3339(),
            updated_at: desc.updated_at.to_rfc3339(),
        }),
    ))
}

// ============ Get Event Descriptions ============

#[derive(Deserialize)]
pub struct ListEventDescriptionsQuery {
    pub workspace_id: WorkspaceId,
}

#[derive(Serialize)]
pub struct ListEventDescriptionsResponse {
    pub event_descriptions: Vec<EventDescriptionResponse>,
}

/// List all event descriptions for a workspace (any workspace member)
pub async fn list_event_descriptions(
    State(state): State<AppState>,
    WorkspaceContext {
        workspace, role, ..
    }: WorkspaceContext,
) -> Result<Json<ListEventDescriptionsResponse>, EventDescriptionError> {
    if role.is_none() {
        return Err(EventDescriptionError::Forbidden);
    }

    let descriptions = db::list_event_descriptions_by_workspace(&state.db_pool, &workspace.id)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    let response: Vec<EventDescriptionResponse> = descriptions
        .into_iter()
        .map(|d| EventDescriptionResponse {
            id: d.id.to_string(),
            workspace_id: d.workspace_id.to_string(),
            event_name: d.event_name,
            description: d.description,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListEventDescriptionsResponse {
        event_descriptions: response,
    }))
}

// ============ Get Single Event Description ============

#[derive(Deserialize)]
pub struct GetEventDescriptionQuery {
    pub workspace_id: WorkspaceId,
    pub event_name: String,
}

/// Get a single event description by workspace_id and event_name
pub async fn get_event_description(
    State(state): State<AppState>,
    Query(query): Query<GetEventDescriptionQuery>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<EventDescriptionResponse>, EventDescriptionError> {
    // Check user has access to this workspace
    let role = db::get_user_workspace_role(&state.db_pool, user.id, &query.workspace_id)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    if role.is_none() {
        return Err(EventDescriptionError::Forbidden);
    }

    let desc = db::get_event_description(&state.db_pool, &query.workspace_id, &query.event_name)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?
        .ok_or(EventDescriptionError::NotFound)?;

    Ok(Json(EventDescriptionResponse {
        id: desc.id.to_string(),
        workspace_id: desc.workspace_id.to_string(),
        event_name: desc.event_name,
        description: desc.description,
        created_at: desc.created_at.to_rfc3339(),
        updated_at: desc.updated_at.to_rfc3339(),
    }))
}

// ============ Errors ============

#[derive(Debug)]
pub enum EventDescriptionError {
    Database(String),
    NotFound,
    Forbidden,
}

impl axum::response::IntoResponse for EventDescriptionError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            EventDescriptionError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            EventDescriptionError::NotFound => (
                StatusCode::NOT_FOUND,
                "Event description not found".to_string(),
            ),
            EventDescriptionError::Forbidden => (
                StatusCode::FORBIDDEN,
                "Insufficient permissions".to_string(),
            ),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
