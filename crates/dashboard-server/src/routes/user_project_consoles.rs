use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    db,
    middleware::ProjectAccess,
    AppState,
};

// ============ Request/Response Types ============

#[derive(Deserialize)]
pub struct CreateConsoleRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub console: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateConsoleRequest {
    pub console_id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub console: Option<String>,
}

#[derive(Deserialize)]
pub struct GetConsoleQuery {
    pub console_id: String,
}

#[derive(Deserialize)]
pub struct DeleteConsoleQuery {
    pub console_id: String,
}

#[derive(Serialize)]
pub struct ConsoleResponse {
    pub id: String,
    pub user_id: String,
    pub proj_id: String,
    pub name: Option<String>,
    pub console: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ============ Create Console ============

pub async fn create_console(
    State(state): State<AppState>,
    ProjectAccess { project, auth_user, .. }: ProjectAccess,
    Json(req): Json<CreateConsoleRequest>,
) -> Result<(StatusCode, Json<ConsoleResponse>), ConsoleError> {
    let console = db::create_console(
        &state.db_pool,
        auth_user.user.id,
        project.id,
        req.name.as_deref(),
        req.console.as_deref(),
    )
    .await
    .map_err(|e| ConsoleError::Database(e.to_string()))?;

    Ok((StatusCode::CREATED, Json(console_to_response(console))))
}

// ============ List My Consoles ============

#[derive(Serialize)]
pub struct ConsoleListItem {
    pub id: String,
    pub name: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn list_consoles(
    State(state): State<AppState>,
    ProjectAccess { project, auth_user, .. }: ProjectAccess,
) -> Result<Json<Vec<ConsoleListItem>>, ConsoleError> {
    let consoles = db::list_consoles_by_user_and_project(
        &state.db_pool,
        auth_user.user.id,
        project.id,
    )
    .await
    .map_err(|e| ConsoleError::Database(e.to_string()))?;

    let response: Vec<ConsoleListItem> = consoles
        .into_iter()
        .map(|c| ConsoleListItem {
            id: c.id.to_string(),
            name: c.name,
            created_at: c.created_at.to_rfc3339(),
            updated_at: c.updated_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

// ============ Get Single Console ============

pub async fn get_console(
    State(state): State<AppState>,
    ProjectAccess { auth_user, .. }: ProjectAccess,
    Query(query): Query<GetConsoleQuery>,
) -> Result<Json<ConsoleResponse>, ConsoleError> {
    let console_id = Uuid::parse_str(&query.console_id)
        .map_err(|_| ConsoleError::InvalidConsoleId)?;

    let console = db::get_console_by_id_and_user(&state.db_pool, console_id, auth_user.user.id)
        .await
        .map_err(|e| ConsoleError::Database(e.to_string()))?
        .ok_or(ConsoleError::NotFound)?;

    Ok(Json(console_to_response(console)))
}

// ============ Update Console ============

pub async fn update_console(
    State(state): State<AppState>,
    ProjectAccess { auth_user, .. }: ProjectAccess,
    Json(req): Json<UpdateConsoleRequest>,
) -> Result<Json<ConsoleResponse>, ConsoleError> {
    let console_id = Uuid::parse_str(&req.console_id)
        .map_err(|_| ConsoleError::InvalidConsoleId)?;

    // Verify ownership
    let existing = db::get_console_by_id_and_user(&state.db_pool, console_id, auth_user.user.id)
        .await
        .map_err(|e| ConsoleError::Database(e.to_string()))?
        .ok_or(ConsoleError::NotFound)?;

    let console = db::update_console(
        &state.db_pool,
        console_id,
        req.name.as_deref().or(existing.name.as_deref()),
        req.console.as_deref().or(existing.console.as_deref()),
    )
    .await
    .map_err(|e| ConsoleError::Database(e.to_string()))?;

    Ok(Json(console_to_response(console)))
}

// ============ Delete Console ============

pub async fn delete_console(
    State(state): State<AppState>,
    ProjectAccess { auth_user, .. }: ProjectAccess,
    Query(query): Query<DeleteConsoleQuery>,
) -> Result<StatusCode, ConsoleError> {
    let console_id = Uuid::parse_str(&query.console_id)
        .map_err(|_| ConsoleError::InvalidConsoleId)?;

    // Verify ownership
    db::get_console_by_id_and_user(&state.db_pool, console_id, auth_user.user.id)
        .await
        .map_err(|e| ConsoleError::Database(e.to_string()))?
        .ok_or(ConsoleError::NotFound)?;

    db::delete_console(&state.db_pool, console_id)
        .await
        .map_err(|e| ConsoleError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

// ============ Helper ============

fn console_to_response(c: db::UserProjectConsole) -> ConsoleResponse {
    ConsoleResponse {
        id: c.id.to_string(),
        user_id: c.user_id.to_string(),
        proj_id: c.proj_id.to_string(),
        name: c.name,
        console: c.console,
        created_at: c.created_at.to_rfc3339(),
        updated_at: c.updated_at.to_rfc3339(),
    }
}

// ============ Errors ============

#[derive(Debug)]
pub enum ConsoleError {
    Database(String),
    NotFound,
    InvalidConsoleId,
}

impl axum::response::IntoResponse for ConsoleError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ConsoleError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error".to_string())
            }
            ConsoleError::NotFound => (StatusCode::NOT_FOUND, "Console not found".to_string()),
            ConsoleError::InvalidConsoleId => (StatusCode::BAD_REQUEST, "Invalid console ID".to_string()),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
