//! Event descriptions, kept by clickhouse-proxy (see `clickhouse_relay`):
//! workspace members read them, workspace admins write them.

use axum::{
    Json,
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use canonical_event::WorkspaceId;
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    clickhouse_relay::relay,
    db,
    middleware::{AuthUser, OrgAdmin, WorkspaceAdmin, WorkspaceContext},
};

// ============ Create/Update Event Description ============

#[derive(Deserialize)]
pub struct UpsertEventDescriptionRequest {
    pub event_name: String,
    pub description: String,
}

async fn upsert(
    state: &AppState,
    workspace: &WorkspaceId,
    req: UpsertEventDescriptionRequest,
) -> Response {
    let body = json!({ "description": req.description }).to_string();
    relay(
        state,
        Method::PUT,
        workspace,
        &["event-descriptions", &req.event_name],
        &[],
        Some(Bytes::from(body)),
    )
    .await
}

/// Create or update event description (requires workspace admin)
pub async fn upsert_event_description(
    State(state): State<AppState>,
    WorkspaceAdmin { workspace, .. }: WorkspaceAdmin,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Response {
    upsert(&state, &workspace.id, req).await
}

/// Create or update event description as org admin (requires X-Workspace-Id header)
pub async fn upsert_event_description_as_org_admin(
    State(state): State<AppState>,
    OrgAdmin { organization, .. }: OrgAdmin,
    WorkspaceContext { workspace, .. }: WorkspaceContext,
    Json(req): Json<UpsertEventDescriptionRequest>,
) -> Response {
    // Verify workspace belongs to this organization
    if workspace.organization_id != Some(organization.id) {
        return forbidden();
    }
    upsert(&state, &workspace.id, req).await
}

// ============ Get Event Descriptions ============

/// List all event descriptions for a workspace (any workspace member)
pub async fn list_event_descriptions(
    State(state): State<AppState>,
    WorkspaceContext {
        workspace, role, ..
    }: WorkspaceContext,
) -> Response {
    if role.is_none() {
        return forbidden();
    }
    relay(
        &state,
        Method::GET,
        &workspace.id,
        &["event-descriptions"],
        &[],
        None,
    )
    .await
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
        &["event-descriptions", &query.event_name],
        &[],
        None,
    )
    .await
}

// ============ Errors ============

pub(crate) fn forbidden() -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({ "error": "Insufficient permissions" })),
    )
        .into_response()
}

pub(crate) fn database_error(error: impl std::fmt::Display) -> Response {
    tracing::error!("Database error: {}", error);
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "Internal server error" })),
    )
        .into_response()
}
