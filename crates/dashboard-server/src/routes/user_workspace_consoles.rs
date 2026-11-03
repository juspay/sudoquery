//! Saved SQL consoles, kept by clickhouse-proxy per workspace and owner (see
//! `clickhouse_relay`). Each person sees only their own.

use axum::{
    body::Bytes,
    extract::{Query, State},
    response::Response,
};
use reqwest::Method;
use serde::Deserialize;
use serde_json::json;

use crate::{AppState, clickhouse_relay::relay, middleware::WorkspaceAccess};

// ============ Request Types ============

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

// ============ Create Console ============

pub async fn create_console(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
    body: Bytes,
) -> Response {
    let owner = auth_user.user.id.to_string();
    relay(
        &state,
        Method::POST,
        &workspace.id,
        &["owners", &owner, "consoles"],
        &[],
        Some(body),
    )
    .await
}

// ============ List My Consoles ============

pub async fn list_consoles(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
) -> Response {
    let owner = auth_user.user.id.to_string();
    relay(
        &state,
        Method::GET,
        &workspace.id,
        &["owners", &owner, "consoles"],
        &[],
        None,
    )
    .await
}

// ============ Get Single Console ============

pub async fn get_console(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
    Query(query): Query<GetConsoleQuery>,
) -> Response {
    let owner = auth_user.user.id.to_string();
    relay(
        &state,
        Method::GET,
        &workspace.id,
        &["owners", &owner, "consoles", &query.console_id],
        &[],
        None,
    )
    .await
}

// ============ Update Console ============

pub async fn update_console(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
    axum::Json(req): axum::Json<UpdateConsoleRequest>,
) -> Response {
    let owner = auth_user.user.id.to_string();
    let body = json!({ "name": req.name, "console": req.console }).to_string();
    relay(
        &state,
        Method::PATCH,
        &workspace.id,
        &["owners", &owner, "consoles", &req.console_id],
        &[],
        Some(Bytes::from(body)),
    )
    .await
}

// ============ Delete Console ============

pub async fn delete_console(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
    Query(query): Query<DeleteConsoleQuery>,
) -> Response {
    let owner = auth_user.user.id.to_string();
    relay(
        &state,
        Method::DELETE,
        &workspace.id,
        &["owners", &owner, "consoles", &query.console_id],
        &[],
        None,
    )
    .await
}
