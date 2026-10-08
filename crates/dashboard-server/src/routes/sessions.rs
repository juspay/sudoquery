use axum::{
    extract::{Path, State},
    response::Json,
};

use crate::{
    AppState,
    middleware::ProjectAccess,
    opensearch::{CursorPage, Event, QueryRequest, SessionSummary, SortOrder},
    routes::search::{SearchError, project_scope},
};

// ============ List Sessions ============

/// The query must hold a bounded `range` on `occured_at`: every page
/// aggregates all the events it matches.
pub async fn list_sessions(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<SessionSummary>>, SearchError> {
    let scope = project_scope(&project)?;

    let page = state.opensearch.list_sessions(&scope, &req).await?;

    Ok(Json(page))
}

// ============ Get Session Events ============

pub async fn get_session(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(session_id): Path<String>,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<Event>>, SearchError> {
    let scope = project_scope(&project)?;

    let page = state
        .opensearch
        .search_events(&scope, &req, Some(&session_id), SortOrder::Asc)
        .await?;

    Ok(Json(page))
}
