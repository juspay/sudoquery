use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    AppState,
    db::{
        self,
        chat::{self, ChatType},
    },
    middleware::WorkspaceAccess,
};

#[derive(Serialize)]
pub struct ChatResponse {
    pub id: String,
    pub workspace_id: String,
    pub user_id: String,
    pub title: String,
    pub chat_type: String,
    pub created_at: String,
}

pub async fn list_chats(
    State(state): State<AppState>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
) -> Result<Json<Vec<ChatResponse>>, ChatError> {
    let chats =
        db::list_chats_by_workspace_and_user(&state.db_pool, &workspace.id, auth_user.user.id)
            .await
            .map_err(|e| ChatError::Database(e.to_string()))?;

    let response: Vec<ChatResponse> = chats
        .into_iter()
        .map(|c| ChatResponse {
            id: c.id.to_string(),
            workspace_id: c.workspace_id.to_string(),
            user_id: c.user_id.to_string(),
            title: c.title,
            chat_type: serde_json::to_string(&c.chat_type)
                .unwrap_or_else(|_| "general".to_string())
                .trim_matches('"')
                .to_string(),
            created_at: c.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(response))
}

pub async fn delete_chat(
    State(state): State<AppState>,
    Path(chat_id): Path<Uuid>,
    WorkspaceAccess {
        workspace,
        auth_user,
        ..
    }: WorkspaceAccess,
) -> Result<StatusCode, ChatError> {
    let chat = match chat::get_chat_by_id(&state.db_pool, chat_id).await {
        Ok(Some(c)) => c,
        Ok(None) => return Ok(StatusCode::NOT_FOUND),
        Err(e) => return Err(ChatError::Database(e.to_string())),
    };

    // Verify the chat belongs to the user's workspace
    if chat.workspace_id != workspace.id {
        return Err(ChatError::Database(
            "Chat not found in this workspace".to_string(),
        ));
    }

    // Optionally verify the user owns the chat (or is admin)
    // Commenting out strict ownership check to allow workspace members to delete
    // if chat.user_id != auth_user.user.id {
    //     return Err(ChatError::Database("Not authorized to delete this chat".to_string()));
    // }

    chat::delete_chat(&state.db_pool, chat_id)
        .await
        .map_err(|e| ChatError::Database(e.to_string()))?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug)]
pub enum ChatError {
    Database(String),
}

impl axum::response::IntoResponse for ChatError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ChatError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}
