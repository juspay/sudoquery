use axum::{extract::State, response::Json};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

use crate::{
    middleware::ProjectAccess,
    opensearch::{Facets, Histogram},
    routes::search::{project_scope, SearchError},
    AppState,
};

// ============ Request Types ============

#[derive(Deserialize)]
pub struct HistogramRequest {
    #[serde(default)]
    pub query: Option<Value>,
    /// Start of the time range, inclusive.
    pub from: DateTime<Utc>,
    /// End of the time range, exclusive.
    pub to: DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct FacetsRequest {
    #[serde(default)]
    pub query: Option<Value>,
    pub field: String,
    #[serde(default)]
    pub size: Option<usize>,
    /// Only count values starting with this text.
    #[serde(default)]
    pub prefix: Option<String>,
}

// ============ Event Histogram ============

pub async fn histogram(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<HistogramRequest>,
) -> Result<Json<Histogram>, SearchError> {
    let scope = project_scope(&project)?;

    let histogram = state
        .opensearch
        .histogram(&scope, req.query.as_ref(), req.from, req.to)
        .await?;

    Ok(Json(histogram))
}

// ============ Field Facets ============

/// The query must hold a bounded `range` on `occured_at`, as for the session
/// list: the aggregation reads every event it matches.
pub async fn facets(
    State(state): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<FacetsRequest>,
) -> Result<Json<Facets>, SearchError> {
    let scope = project_scope(&project)?;

    let facets = state
        .opensearch
        .facets(
            &scope,
            req.query.as_ref(),
            &req.field,
            req.size,
            req.prefix.as_deref(),
        )
        .await?;

    Ok(Json(facets))
}
