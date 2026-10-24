use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::{
    AppState,
    middleware::WorkspaceAccess,
    opensearch::{CursorPage, Event, QueryRequest, SessionSummary, SortOrder},
    routes::search::{SearchError, workspace_scope},
};

// ============ List Sessions ============

/// The query must hold a bounded `range` on `occured_at`: every page
/// aggregates all the events it matches.
pub async fn list_sessions(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<SessionSummary>>, SearchError> {
    let scope = workspace_scope(&workspace)?;

    let page = state.opensearch.list_sessions(&scope, &req).await?;

    Ok(Json(page))
}

// ============ Get Session Events ============

pub async fn get_session(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    Path(session_id): Path<String>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<Event>>, SearchError> {
    let scope = workspace_scope(&workspace)?;

    let page = state
        .opensearch
        .search_events(&scope, &req, Some(&session_id), SortOrder::Asc)
        .await?;

    Ok(Json(page))
}
