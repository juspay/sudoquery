//! Event histogram and field facets, served by opensearch-proxy (see
//! `search_proxy`).

use axum::{body::Bytes, extract::State, response::Response};
use reqwest::Method;

use crate::{AppState, middleware::WorkspaceAccess};

// ============ Event Histogram ============

pub async fn histogram(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(Method::POST, &workspace, &["histogram"], Some(body))
        .await
}

// ============ Field Facets ============

/// The query must hold a bounded `range` on `occured_at`, as for the session
/// list: the aggregation reads every event it matches.
pub async fn facets(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(Method::POST, &workspace, &["facets"], Some(body))
        .await
}
