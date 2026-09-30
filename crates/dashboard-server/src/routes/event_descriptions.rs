use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db,
    middleware::{AuthUser, OrgAdmin, OrgContext, ProjectAdmin, ProjectContext},
    AppState,
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
    pub project_id: String,
    pub event_name: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Create or update event description (requires project admin)
pub async fn upsert_event_description(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Result<(StatusCode, Json<EventDescriptionResponse>), EventDescriptionError> {
    let desc = db::upsert_event_description(
        &state.db_pool,
        project.id,
        &req.event_name,
        &req.description,
    )
    .await
    .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    Ok((StatusCode::OK, Json(EventDescriptionResponse {
        id: desc.id.to_string(),
        project_id: desc.project_id.to_string(),
        event_name: desc.event_name,
        description: desc.description,
        created_at: desc.created_at.to_rfc3339(),
        updated_at: desc.updated_at.to_rfc3339(),
    })))
}

/// Create or update event description as org admin (requires X-Project-Id header)
pub async fn upsert_event_description_as_org_admin(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    ProjectContext { project, .. }: ProjectContext,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Result<(StatusCode, Json<EventDescriptionResponse>), EventDescriptionError> {
    // Verify project belongs to this organization
    if project.organization_id != Some(organization.id) {
        return Err(EventDescriptionError::Forbidden);
    }

    let desc = db::upsert_event_description(
        &state.db_pool,
        project.id,
        &req.event_name,
        &req.description,
    )
    .await
    .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    Ok((StatusCode::OK, Json(EventDescriptionResponse {
        id: desc.id.to_string(),
        project_id: desc.project_id.to_string(),
        event_name: desc.event_name,
        description: desc.description,
        created_at: desc.created_at.to_rfc3339(),
        updated_at: desc.updated_at.to_rfc3339(),
    })))
}

// ============ Get Event Descriptions ============

#[derive(Deserialize)]
pub struct ListEventDescriptionsQuery {
    pub project_id: Uuid,
}

#[derive(Serialize)]
pub struct ListEventDescriptionsResponse {
    pub event_descriptions: Vec<EventDescriptionResponse>,
}

/// List all event descriptions for a project (any project member)
pub async fn list_event_descriptions(
    State(state): State<AppState>,
    ProjectContext { project, .. }: ProjectContext,
) -> Result<Json<ListEventDescriptionsResponse>, EventDescriptionError> {
    let descriptions = db::list_event_descriptions_by_project(&state.db_pool, project.id)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    let response: Vec<EventDescriptionResponse> = descriptions
        .into_iter()
        .map(|d| EventDescriptionResponse {
            id: d.id.to_string(),
            project_id: d.project_id.to_string(),
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
    pub project_id: Uuid,
    pub event_name: String,
}

/// Get a single event description by project_id and event_name
pub async fn get_event_description(
    State(state): State<AppState>,
    Query(query): Query<GetEventDescriptionQuery>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<EventDescriptionResponse>, EventDescriptionError> {
    // Check user has access to this project
    let role = db::get_user_project_role(&state.db_pool, user.id, query.project_id)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?;

    if role.is_none() {
        return Err(EventDescriptionError::Forbidden);
    }

    let desc = db::get_event_description(&state.db_pool, query.project_id, &query.event_name)
        .await
        .map_err(|e| EventDescriptionError::Database(e.to_string()))?
        .ok_or(EventDescriptionError::NotFound)?;

    Ok(Json(EventDescriptionResponse {
        id: desc.id.to_string(),
        project_id: desc.project_id.to_string(),
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
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            EventDescriptionError::NotFound => (StatusCode::NOT_FOUND, "Event description not found".to_string()),
            EventDescriptionError::Forbidden => (StatusCode::FORBIDDEN, "Insufficient permissions".to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
