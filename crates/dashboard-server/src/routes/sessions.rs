//! Sessions, served by opensearch-proxy (see `search_proxy`).

use axum::{
    body::Bytes,
    extract::{Path, State},
    response::Response,
};
use reqwest::Method;

use crate::{AppState, middleware::WorkspaceAccess};

// ============ List Sessions ============

/// The query must hold a bounded `range` on `occured_at`: every page
/// aggregates all the events it matches.
pub async fn list_sessions(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(Method::POST, &workspace, &["sessions"], Some(body))
        .await
}

// ============ Get Session Events ============

pub async fn get_session(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    Path(session_id): Path<String>,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(
            Method::POST,
            &workspace,
            &["sessions", &session_id],
            Some(body),
        )
        .await
}
