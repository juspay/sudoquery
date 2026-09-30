use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    db::{self, message},
    middleware::ProjectAdmin,
};

// ============ Request/Response Types ============

#[derive(Deserialize)]
pub struct SaveLiveDashboardRequest {
    pub message_id: Uuid,
    pub tool_call_id: String,
}

#[derive(Serialize)]
pub struct SaveLiveDashboardResponse {
    pub dashboard_id: String,
}

// ============ Save Live Dashboard from Tool Call ============

pub async fn save_live_dashboard_from_tool_call(
    State(state): State<AppState>,
    ProjectAdmin { project, .. }: ProjectAdmin,
    Path(chat_id): Path<Uuid>,
    Json(req): Json<SaveLiveDashboardRequest>,
) -> Result<(StatusCode, Json<SaveLiveDashboardResponse>), SaveDashboardError> {
    // 1. Verify the message exists and belongs to the chat
    let message = message::get_message_by_id(&state.db_pool, req.message_id)
        .await
        .map_err(|e| SaveDashboardError::Database(e.to_string()))?
        .ok_or(SaveDashboardError::MessageNotFound)?;

    if message.chat_id != chat_id {
        return Err(SaveDashboardError::MessageNotFound);
    }

    // 2. Extract tool calls from the message
    let tool_calls = message
        .message
        .get("tool_calls")
        .and_then(|v| v.as_array())
        .ok_or(SaveDashboardError::InvalidToolCall)?;

    // 3. Find the specific tool call by ID
    let tool_call = tool_calls
        .iter()
        .find(|tc| {
            tc.get("id")
                .and_then(|v| v.as_str())
                .map(|id| id == req.tool_call_id)
                .unwrap_or(false)
        })
        .ok_or(SaveDashboardError::ToolCallNotFound)?;

    // 4. Verify it's the test_run_live_dashboard tool
    let function_name = tool_call
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(|n| n.as_str())
        .ok_or(SaveDashboardError::InvalidToolCall)?;

    if function_name != "test_run_live_dashboard" {
        return Err(SaveDashboardError::InvalidToolType);
    }

    // 5. Parse the tool call arguments
    let arguments_str = tool_call
        .get("function")
        .and_then(|f| f.get("arguments"))
        .and_then(|a| a.as_str())
        .ok_or(SaveDashboardError::InvalidToolCall)?;

    let args: serde_json::Value =
        serde_json::from_str(arguments_str).map_err(|_| SaveDashboardError::InvalidToolCall)?;

    // 6. Extract query from the tool call arguments
    // The test_run_live_dashboard tool expects query_tool_call_id to reference a previous query
    let query_tool_call_id = args
        .get("query_tool_call_id")
        .and_then(|v| v.as_str())
        .ok_or(SaveDashboardError::MissingQueryToolCallId)?;

    // 7. Find the query from the referenced tool call
    let query = find_query_in_chat_history(&state.db_pool, chat_id, query_tool_call_id).await?;

    // 8. Extract description, label (title), and chart_config
    let description = args
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("Dashboard created from chat");

    let title = args.get("label").and_then(|v| v.as_str());

    let chart_config = args.get("chart_config").map(|v| v.to_string());

    // 9. Check if dashboard already exists with this creation_source
    let creation_source = format!("{}-{}", req.message_id, req.tool_call_id);
    if let Some(existing) =
        db::live_dashboard::get_dashboard_by_source(&state.db_pool, project.id, &creation_source)
            .await
            .map_err(|e| SaveDashboardError::Database(e.to_string()))?
    {
        return Err(SaveDashboardError::DuplicateDashboard(existing.id));
    }

    // 10. Create the dashboard
    let dashboard = db::live_dashboard::create_dashboard(
        &state.db_pool,
        project.id,
        &query,
        description,
        title,
        chart_config.as_deref(),
        Some(&creation_source),
    )
    .await
    .map_err(|e| SaveDashboardError::Database(e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(SaveLiveDashboardResponse {
            dashboard_id: dashboard.id.to_string(),
        }),
    ))
}

// Helper function to find a query from a tool call in chat history
async fn find_query_in_chat_history(
    pool: &sqlx::PgPool,
    chat_id: Uuid,
    tool_call_id: &str,
) -> Result<String, SaveDashboardError> {
    // Get all messages in the chat
    let messages = message::list_messages_by_chat(pool, chat_id)
        .await
        .map_err(|e| SaveDashboardError::Database(e.to_string()))?;

    // Look through messages to find the tool call with the given ID
    for msg in messages {
        if let Some(tool_calls) = msg.message.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tool_calls {
                let tc_id = tc.get("id").and_then(|v| v.as_str());
                if tc_id == Some(tool_call_id) {
                    // Found the tool call, extract the query
                    let function_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");

                    if function_name == "execute_clickhouse_query" {
                        let args_str = tc
                            .get("function")
                            .and_then(|f| f.get("arguments"))
                            .and_then(|a| a.as_str())
                            .ok_or(SaveDashboardError::InvalidToolCall)?;

                        let args: serde_json::Value = serde_json::from_str(args_str)
                            .map_err(|_| SaveDashboardError::InvalidToolCall)?;

                        let query = args
                            .get("query")
                            .and_then(|v| v.as_str())
                            .ok_or(SaveDashboardError::MissingQuery)?;

                        return Ok(query.to_string());
                    }
                }
            }
        }
    }

    Err(SaveDashboardError::QueryToolCallNotFound)
}

// ============ Errors ============

#[derive(Debug)]
pub enum SaveDashboardError {
    Database(String),
    MessageNotFound,
    ToolCallNotFound,
    InvalidToolCall,
    InvalidToolType,
    MissingQueryToolCallId,
    QueryToolCallNotFound,
    MissingQuery,
    DuplicateDashboard(Uuid),
}

impl axum::response::IntoResponse for SaveDashboardError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            SaveDashboardError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            SaveDashboardError::MessageNotFound => {
                (StatusCode::NOT_FOUND, "Message not found".to_string())
            }
            SaveDashboardError::ToolCallNotFound => (
                StatusCode::NOT_FOUND,
                "Tool call not found in message".to_string(),
            ),
            SaveDashboardError::InvalidToolCall => (
                StatusCode::BAD_REQUEST,
                "Invalid tool call structure".to_string(),
            ),
            SaveDashboardError::InvalidToolType => (
                StatusCode::BAD_REQUEST,
                "Tool call is not test_run_live_dashboard".to_string(),
            ),
            SaveDashboardError::MissingQueryToolCallId => (
                StatusCode::BAD_REQUEST,
                "Missing query_tool_call_id in tool call".to_string(),
            ),
            SaveDashboardError::QueryToolCallNotFound => (
                StatusCode::NOT_FOUND,
                "Referenced query tool call not found in chat history".to_string(),
            ),
            SaveDashboardError::MissingQuery => (
                StatusCode::BAD_REQUEST,
                "Query not found in referenced tool call".to_string(),
            ),
            SaveDashboardError::DuplicateDashboard(id) => (
                StatusCode::CONFLICT,
                format!("Dashboard already exists: {}", id),
            ),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
