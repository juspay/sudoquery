//! Event search, served by opensearch-proxy: the caller's workspace access is
//! checked here, the search runs there (see `search_proxy`).

use axum::{
    body::Bytes,
    extract::{Path, State},
    response::Response,
};
use reqwest::Method;

use crate::{AppState, middleware::WorkspaceAccess};

// ============ Search Events ============

pub async fn search(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(Method::POST, &workspace, &["search"], Some(body))
        .await
}

// ============ Count Events ============

pub async fn count(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    state
        .search_proxy
        .forward(Method::POST, &workspace, &["count"], Some(body))
        .await
}

// ============ Get Single Event ============

pub async fn get_doc(
    State(state): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    Path(doc_id): Path<String>,
) -> Response {
    state
        .search_proxy
        .forward(Method::GET, &workspace, &["doc", &doc_id], None)
        .await
}
