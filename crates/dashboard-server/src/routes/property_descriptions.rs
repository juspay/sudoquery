use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use canonical_event::ProjectId;
use serde::{Deserialize, Serialize};

use crate::{
    AppState, db,
    middleware::{AuthUser, ProjectAdmin, ProjectContext},
};

// ============ Create/Update Property Description ============

#[derive(Deserialize)]
pub struct UpsertPropertyDescriptionRequest {
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct PropertyDescriptionResponse {
    pub id: String,
    pub project_id: String,
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Create or update property description (requires project admin)
pub async fn upsert_property_description(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Json(req): Json<UpsertPropertyDescriptionRequest>,
) -> Result<(StatusCode, Json<PropertyDescriptionResponse>), PropertyDescriptionError> {
    let desc = db::upsert_property_description(
        &state.db_pool,
        &project.id,
        &req.event_name,
        &req.property_name,
        &req.property_type,
        &req.description,
    )
    .await
    .map_err(|e| PropertyDescriptionError::Database(e.to_string()))?;

    Ok((
        StatusCode::OK,
        Json(PropertyDescriptionResponse {
            id: desc.id.to_string(),
            project_id: desc.project_id.to_string(),
            event_name: desc.event_name,
            property_name: desc.property_name,
            property_type: desc.property_type,
            description: desc.description,
            created_at: desc.created_at.to_rfc3339(),
            updated_at: desc.updated_at.to_rfc3339(),
        }),
    ))
}

// ============ Get Property Descriptions ============

#[derive(Deserialize)]
pub struct ListPropertyDescriptionsQuery {
    pub event_name: Option<String>,
}

#[derive(Serialize)]
pub struct ListPropertyDescriptionsResponse {
    pub property_descriptions: Vec<PropertyDescriptionResponse>,
}

/// List property descriptions for a project
/// - If event_name is provided, filters by event
/// - Otherwise returns all property descriptions for the project
pub async fn list_property_descriptions(
    State(state): State<AppState>,
    ProjectContext { project, .. }: ProjectContext,
    Query(query): Query<ListPropertyDescriptionsQuery>,
) -> Result<Json<ListPropertyDescriptionsResponse>, PropertyDescriptionError> {
    let descriptions = if let Some(event_name) = query.event_name {
        db::list_property_descriptions_by_event(&state.db_pool, &project.id, &event_name)
            .await
            .map_err(|e| PropertyDescriptionError::Database(e.to_string()))?
    } else {
        db::list_property_descriptions_by_project(&state.db_pool, &project.id)
            .await
            .map_err(|e| PropertyDescriptionError::Database(e.to_string()))?
    };

    let response: Vec<PropertyDescriptionResponse> = descriptions
        .into_iter()
        .map(|d| PropertyDescriptionResponse {
            id: d.id.to_string(),
            project_id: d.project_id.to_string(),
            event_name: d.event_name,
            property_name: d.property_name,
            property_type: d.property_type,
            description: d.description,
            created_at: d.created_at.to_rfc3339(),
            updated_at: d.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListPropertyDescriptionsResponse {
        property_descriptions: response,
    }))
}

// ============ Get Single Property Description ============

#[derive(Deserialize)]
pub struct GetPropertyDescriptionQuery {
    pub project_id: ProjectId,
    pub event_name: String,
    pub property_name: String,
}

/// Get a single property description by project_id, event_name, and property_name
pub async fn get_property_description(
    State(state): State<AppState>,
    Query(query): Query<GetPropertyDescriptionQuery>,
    AuthUser { user, .. }: AuthUser,
) -> Result<Json<PropertyDescriptionResponse>, PropertyDescriptionError> {
    // Check user has access to this project
    let role = db::get_user_project_role(&state.db_pool, user.id, &query.project_id)
        .await
        .map_err(|e| PropertyDescriptionError::Database(e.to_string()))?;

    if role.is_none() {
        return Err(PropertyDescriptionError::Forbidden);
    }

    let desc = db::get_property_description(
        &state.db_pool,
        &query.project_id,
        &query.event_name,
        &query.property_name,
    )
    .await
    .map_err(|e| PropertyDescriptionError::Database(e.to_string()))?
    .ok_or(PropertyDescriptionError::NotFound)?;

    Ok(Json(PropertyDescriptionResponse {
        id: desc.id.to_string(),
        project_id: desc.project_id.to_string(),
        event_name: desc.event_name,
        property_name: desc.property_name,
        property_type: desc.property_type,
        description: desc.description,
        created_at: desc.created_at.to_rfc3339(),
        updated_at: desc.updated_at.to_rfc3339(),
    }))
}

// ============ Errors ============

#[derive(Debug)]
pub enum PropertyDescriptionError {
    Database(String),
    NotFound,
    Forbidden,
}

impl axum::response::IntoResponse for PropertyDescriptionError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            PropertyDescriptionError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            PropertyDescriptionError::NotFound => (
                StatusCode::NOT_FOUND,
                "Property description not found".to_string(),
            ),
            PropertyDescriptionError::Forbidden => (
                StatusCode::FORBIDDEN,
                "Insufficient permissions".to_string(),
            ),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
