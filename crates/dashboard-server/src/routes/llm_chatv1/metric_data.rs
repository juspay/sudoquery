use axum::{Json, extract::State, http::StatusCode};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    AppState,
    db::{chat, message},
    middleware::{AuthUser, ProjectAccess},
};

#[derive(serde::Deserialize)]
pub struct MetricRequest {
    chat_id: Uuid,
    message_id: Uuid,
    tool_call_id: String,
}

#[derive(serde::Serialize)]
pub struct MetricResponse {
    data: Value,
}

pub async fn get_metric_data_handler(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<MetricRequest>,
) -> Result<Json<MetricResponse>, StatusCode> {
    let chat = chat::get_chat_by_id(&state.db_pool, req.chat_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if chat.project_id != project.id {
        return Err(StatusCode::FORBIDDEN);
    }

    let msg = message::get_message_by_id(&state.db_pool, req.message_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if msg.chat_id != req.chat_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    if msg.role != "tool" {
        return Err(StatusCode::BAD_REQUEST);
    }

    let message_obj = msg
        .message
        .as_object()
        .ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let msg_tool_call_id = message_obj
        .get("tool_call_id")
        .and_then(|v| v.as_str())
        .ok_or(StatusCode::BAD_REQUEST)?;

    if msg_tool_call_id != req.tool_call_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    let content = message_obj.get("content").ok_or(StatusCode::BAD_REQUEST)?;

    let data: Value = serde_json::from_str(content.as_str().unwrap_or("{}"))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(MetricResponse { data }))
}
