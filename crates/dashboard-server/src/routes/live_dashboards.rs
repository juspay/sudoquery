use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    AppState, clickhouse, db,
    middleware::{ProjectAdmin, ProjectContext},
};

// ============ Request/Response Types ============

#[derive(Deserialize)]
pub struct CreateDashboardRequest {
    pub query: String,
    pub description: String,
    #[serde(default)]
    pub chart_config: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateDashboardRequest {
    pub dashboard_id: String,
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub chart_config: Option<String>,
}

#[derive(Deserialize)]
pub struct GetDashboardQuery {
    pub dashboard_id: String,
}

#[derive(Deserialize)]
pub struct DeleteDashboardQuery {
    pub dashboard_id: String,
}

#[derive(Deserialize)]
pub struct TestRunDashboardRequest {
    pub query: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub chart_config: Option<String>,
}

#[derive(Serialize)]
pub struct DashboardResponse {
    pub id: String,
    pub project_id: String,
    pub query: String,
    pub description: String,
    pub title: Option<String>,
    pub chart_config: Option<String>,
    pub last_ran_at: Option<String>,
    pub response: Option<JsonValue>,
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct ListDashboardsResponse {
    pub dashboards: Vec<DashboardResponse>,
}

// ============ Create Dashboard ============

pub async fn create_dashboard(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Json(req): Json<CreateDashboardRequest>,
) -> Result<(StatusCode, Json<DashboardResponse>), DashboardError> {
    let dashboard = db::create_dashboard(
        &state.db_pool,
        &project.id,
        &req.query,
        &req.description,
        None, // title - deprecated, not accepted
        req.chart_config.as_deref(),
        None, // creation_source - only populated when saving from chat
    )
    .await
    .map_err(|e| DashboardError::Database(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(dashboard_to_response(dashboard))))
}

// ============ List Dashboards ============

pub async fn list_dashboards(
    State(state): State<AppState>,
    ProjectContext { project, .. }: ProjectContext,
) -> Result<Json<ListDashboardsResponse>, DashboardError> {
    let dashboards = db::get_dashboards(&state.db_pool, &project.id)
        .await
        .map_err(|e| DashboardError::Database(e.to_string()))?;

    let response: Vec<DashboardResponse> =
        dashboards.into_iter().map(dashboard_to_response).collect();

    Ok(Json(ListDashboardsResponse {
        dashboards: response,
    }))
}

// ============ Get Dashboard ============

pub async fn get_dashboard(
    State(state): State<AppState>,
    ProjectContext { project, .. }: ProjectContext,
    Query(query): Query<GetDashboardQuery>,
) -> Result<Json<DashboardResponse>, DashboardError> {
    let dashboard_id =
        Uuid::parse_str(&query.dashboard_id).map_err(|_| DashboardError::InvalidDashboardId)?;

    let mut dashboard = db::get_dashboard(&state.db_pool, dashboard_id)
        .await
        .map_err(|e| DashboardError::Database(e.to_string()))?
        .ok_or(DashboardError::NotFound)?;

    // Verify dashboard belongs to the project
    if dashboard.project_id != project.id {
        return Err(DashboardError::NotFound);
    }

    // Check if we need to run the query (never run or last run > 30 seconds ago)
    let should_run = match dashboard.last_ran_at {
        None => true,
        Some(last_ran) => {
            let now = Utc::now();
            let elapsed = now.signed_duration_since(last_ran);
            elapsed.num_seconds() > 30
        }
    };

    let mut error = None;

    if should_run {
        // Execute query against ClickHouse
        match clickhouse::execute_project_query(
            &state.clickhouse_url,
            &project.id,
            &state.clickhouse_project_password,
            &dashboard.query,
        )
        .await
        {
            Ok(response) => {
                // Update dashboard with new response
                dashboard = db::update_dashboard_run(&state.db_pool, dashboard_id, &response)
                    .await
                    .map_err(|e| DashboardError::Database(e.to_string()))?;
            }
            Err(e) => {
                tracing::error!(
                    "ClickHouse query error for dashboard {}: {}",
                    dashboard_id,
                    e
                );
                error = Some(e.to_string());
            }
        }
    }

    let mut response = dashboard_to_response(dashboard);
    response.error = error;
    Ok(Json(response))
}

// ============ Update Dashboard ============

pub async fn update_dashboard(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Json(req): Json<UpdateDashboardRequest>,
) -> Result<Json<DashboardResponse>, DashboardError> {
    let dashboard_id =
        Uuid::parse_str(&req.dashboard_id).map_err(|_| DashboardError::InvalidDashboardId)?;

    // Verify dashboard belongs to the project
    let existing = db::get_dashboard(&state.db_pool, dashboard_id)
        .await
        .map_err(|e| DashboardError::Database(e.to_string()))?
        .ok_or(DashboardError::NotFound)?;

    if existing.project_id != project.id {
        return Err(DashboardError::NotFound);
    }

    let dashboard = db::update_dashboard(
        &state.db_pool,
        dashboard_id,
        req.query.as_deref().unwrap_or(&existing.query),
        req.description.as_deref().unwrap_or(&existing.description),
        existing.title.as_deref(), // Preserve existing title
        req.chart_config
            .as_deref()
            .or(existing.chart_config.as_deref()),
    )
    .await
    .map_err(|e| DashboardError::Database(e.to_string()))?;

    Ok(Json(dashboard_to_response(dashboard)))
}

// ============ Delete Dashboard ============

pub async fn delete_dashboard(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Query(query): Query<DeleteDashboardQuery>,
) -> Result<StatusCode, DashboardError> {
    let dashboard_id =
        Uuid::parse_str(&query.dashboard_id).map_err(|_| DashboardError::InvalidDashboardId)?;

    // Verify dashboard belongs to the project
    let existing = db::get_dashboard(&state.db_pool, dashboard_id)
        .await
        .map_err(|e| DashboardError::Database(e.to_string()))?
        .ok_or(DashboardError::NotFound)?;

    if existing.project_id != project.id {
        return Err(DashboardError::NotFound);
    }

    db::delete_dashboard(&state.db_pool, dashboard_id)
        .await
        .map_err(|e| DashboardError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Test Run Dashboard ============

pub async fn test_run_dashboard(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Json(req): Json<TestRunDashboardRequest>,
) -> Result<Json<DashboardResponse>, DashboardError> {
    // Execute query against ClickHouse
    let response = clickhouse::execute_project_query(
        &state.clickhouse_url,
        &project.id,
        &state.clickhouse_project_password,
        &req.query,
    )
    .await;

    // Build response with query result or error
    let now = Utc::now();
    let (response_value, error) = match response {
        Ok(data) => (Some(data), None),
        Err(e) => {
            tracing::error!("ClickHouse query error: {}", e);
            (None, Some(e.to_string()))
        }
    };

    Ok(Json(DashboardResponse {
        id: String::new(),
        project_id: project.id.to_string(),
        query: req.query,
        description: req.description.unwrap_or_default(),
        title: None,
        chart_config: req.chart_config,
        last_ran_at: Some(now.to_rfc3339()),
        response: response_value,
        created_at: now.to_rfc3339(),
        updated_at: now.to_rfc3339(),
        error,
    }))
}

// ============ Helper ============

fn dashboard_to_response(d: db::LiveDashboard) -> DashboardResponse {
    DashboardResponse {
        id: d.id.to_string(),
        project_id: d.project_id.to_string(),
        query: d.query,
        description: d.description,
        title: d.title,
        chart_config: d.chart_config,
        last_ran_at: d.last_ran_at.map(|t| t.to_rfc3339()),
        response: d.response,
        error: None,
        created_at: d.created_at.to_rfc3339(),
        updated_at: d.updated_at.to_rfc3339(),
    }
}

// ============ Errors ============

#[derive(Debug)]
pub enum DashboardError {
    Database(String),
    NotFound,
    InvalidDashboardId,
}

impl axum::response::IntoResponse for DashboardError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            DashboardError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            DashboardError::NotFound => (StatusCode::NOT_FOUND, "Dashboard not found".to_string()),
            DashboardError::InvalidDashboardId => {
                (StatusCode::BAD_REQUEST, "Invalid dashboard ID".to_string())
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
