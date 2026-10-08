use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    AppState, db,
    middleware::ProjectAccess,
    opensearch::{CursorPage, Event, OpenSearchError, QueryRequest, Scope, SortOrder},
};

// ============ Request/Response Types ============

#[derive(Deserialize)]
pub struct CountRequest {
    #[serde(default)]
    pub query: Option<Value>,
}

#[derive(Serialize)]
pub struct CountResponse {
    pub count: u64,
}

// ============ Search Events ============

pub async fn search(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<QueryRequest>,
) -> Result<Json<CursorPage<Event>>, SearchError> {
    let scope = project_scope(&project)?;

    let page = state
        .opensearch
        .search_events(&scope, &req, None, SortOrder::Desc)
        .await?;

    Ok(Json(page))
}

// ============ Count Events ============

pub async fn count(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<CountRequest>,
) -> Result<Json<CountResponse>, SearchError> {
    let scope = project_scope(&project)?;

    let count = state.opensearch.count(&scope, req.query.as_ref()).await?;

    Ok(Json(CountResponse { count }))
}

// ============ Get Single Event ============

pub async fn get_doc(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Path(doc_id): Path<String>,
) -> Result<Json<Event>, SearchError> {
    let scope = project_scope(&project)?;

    let event = state
        .opensearch
        .get_event(&scope, &doc_id)
        .await?
        .ok_or(SearchError::NotFound)?;

    Ok(Json(event))
}

// ============ Helper ============

/// The events a project may read. Events are stored per organization, so a
/// project outside any organization has none.
pub fn project_scope(project: &db::Project) -> Result<Scope, SearchError> {
    Scope::for_project(project).ok_or(SearchError::NoOrganization)
}

// ============ Errors ============

#[derive(Debug)]
pub enum SearchError {
    NoOrganization,
    NotFound,
    OpenSearch(OpenSearchError),
}

impl From<OpenSearchError> for SearchError {
    fn from(error: OpenSearchError) -> Self {
        SearchError::OpenSearch(error)
    }
}

impl axum::response::IntoResponse for SearchError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            SearchError::NoOrganization => (
                StatusCode::BAD_REQUEST,
                "Project does not belong to an organization".to_string(),
            ),
            SearchError::NotFound => (StatusCode::NOT_FOUND, "Document not found".to_string()),
            SearchError::OpenSearch(OpenSearchError::Request(error)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            SearchError::OpenSearch(error) => {
                tracing::error!("OpenSearch error: {}", error);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
