//! Property descriptions, kept by clickhouse-proxy (see `clickhouse_relay`):
//! workspace members read them, workspace admins write them.

use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    response::Response,
};
use canonical_event::WorkspaceId;
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    clickhouse_relay::relay,
    db,
    middleware::{AuthUser, WorkspaceAdmin, WorkspaceContext},
    routes::event_descriptions::{database_error, forbidden},
};

// ============ Create/Update Property Description ============

#[derive(Deserialize)]
pub struct UpsertPropertyDescriptionRequest {
    pub event_name: String,
    pub property_name: String,
    pub property_type: String,
    pub description: String,
}

/// Create or update property description (requires workspace admin)
pub async fn upsert_property_description(
    State(state): State<AppState>,
    WorkspaceAdmin { workspace, .. }: WorkspaceAdmin,
    Json(req): Json<UpsertPropertyDescriptionRequest>,
) -> Response {
    let body =
        json!({ "property_type": req.property_type, "description": req.description }).to_string();
    relay(
        &state,
        Method::PUT,
        &workspace.id,
        &["property-descriptions", &req.event_name, &req.property_name],
        &[],
        Some(Bytes::from(body)),
    )
    .await
}

// ============ Get Property Descriptions ============

#[derive(Deserialize)]
pub struct ListPropertyDescriptionsQuery {
    pub event_name: Option<String>,
}

/// List property descriptions for a workspace
/// - If event_name is provided, filters by event
/// - Otherwise returns all property descriptions for the workspace
pub async fn list_property_descriptions(
    State(state): State<AppState>,
    WorkspaceContext {
        workspace, role, ..
    }: WorkspaceContext,
    Query(query): Query<ListPropertyDescriptionsQuery>,
) -> Response {
    if role.is_none() {
        return forbidden();
    }
    let filter: Vec<(&str, &str)> = query
        .event_name
        .as_deref()
        .map(|name| ("event_name", name))
        .into_iter()
        .collect();
    relay(
        &state,
        Method::GET,
        &workspace.id,
        &["property-descriptions"],
        &filter,
        None,
    )
    .await
}

// ============ Get Single Property Description ============

#[derive(Deserialize)]
pub struct GetPropertyDescriptionQuery {
    pub workspace_id: WorkspaceId,
    pub event_name: String,
    pub property_name: String,
}

/// Get a single property description by workspace_id, event_name, and property_name
pub async fn get_property_description(
    State(state): State<AppState>,
    Query(query): Query<GetPropertyDescriptionQuery>,
    AuthUser { user, .. }: AuthUser,
) -> Response {
    // Check user has access to this workspace
    match db::get_user_workspace_role(&state.db_pool, user.id, &query.workspace_id).await {
        Ok(Some(_)) => {}
        Ok(None) => return forbidden(),
        Err(e) => return database_error(e),
    }
    relay(
        &state,
        Method::GET,
        &query.workspace_id,
        &[
            "property-descriptions",
            &query.event_name,
            &query.property_name,
        ],
        &[],
        None,
    )
    .await
}
