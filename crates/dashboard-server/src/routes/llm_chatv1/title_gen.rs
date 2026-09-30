use axum::{extract::State, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{
    AppState,
    db::{chat, message},
    middleware::{ProjectAccess, auth::AuthUser},
};

use super::types::{ChatMessage, ChatResponse};

#[derive(Deserialize)]
pub struct GenerateTitleRequest {
    chat_id: Uuid,
}

#[derive(Serialize)]
pub struct GenerateTitleResponse {
    title: String,
}

#[derive(Debug)]
pub enum TitleGenError {
    Database(String),
    NotFound(String),
    Forbidden(String),
}

impl axum::response::IntoResponse for TitleGenError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            TitleGenError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            TitleGenError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            TitleGenError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

const TITLE_GENERATION_SYSTEM_PROMPT: &str = r#"You generate concise conversation titles. Review the conversation and call the generate_title tool with a short title (max 5 words). Do not write any other text."#;

const GENERATE_TITLE_TOOL: &str = r#"{
    "type": "function",
    "function": {
        "name": "generate_title",
        "description": "Generate a concise title for the conversation. Call this once with the title.",
        "parameters": {
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "A concise, descriptive title for the conversation (max 5 words)"
                }
            },
            "required": ["title"]
        }
    }
}"#;

pub async fn generate_title_handler(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<GenerateTitleRequest>,
) -> Result<Json<GenerateTitleResponse>, TitleGenError> {
    let chat_id = req.chat_id;
    
    // Verify chat exists and belongs to this project
    let chat = match chat::get_chat_by_id(&state.db_pool, chat_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err(TitleGenError::NotFound("Chat not found".to_string()));
        }
        Err(e) => {
            return Err(TitleGenError::Database(e.to_string()));
        }
    };
    
    if chat.project_id != project.id {
        return Err(TitleGenError::Forbidden("Chat not found in this project".to_string()));
    }
    
    let messages_result = message::list_messages_by_chat(&state.db_pool, chat_id).await;
    
    let db_messages = match messages_result {
        Ok(msgs) => msgs,
        Err(e) => {
            return Err(TitleGenError::Database(e.to_string()));
        }
    };

    if db_messages.is_empty() {
        return Ok(Json(GenerateTitleResponse {
            title: "New Chat".to_string(),
        }));
    }

    let chat_messages: Vec<ChatMessage> = db_messages.iter().filter_map(|m| {
        let msg = m.message.as_object()?;
        let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
        let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
        
        Some(match role {
            "system" => ChatMessage::new_system(content),
            "user" => ChatMessage::new_user(content),
            "assistant" => ChatMessage::new_assistant(content),
            _ => ChatMessage::new_user(content),
        })
    }).collect();

    if chat_messages.is_empty() {
        return Ok(Json(GenerateTitleResponse {
            title: "New Chat".to_string(),
        }));
    }

    let last_user_message = chat_messages.iter().rev().find(|m| {
        matches!(m, ChatMessage::User(_))
    });

    let conversation_context: Vec<String> = chat_messages.iter().rev().take(6).rev().map(|m| {
        let (role, content) = match m {
            ChatMessage::System(sm) => ("system", sm.content.clone()),
            ChatMessage::User(um) => ("user", um.content.clone()),
            ChatMessage::Assistant(am) => ("assistant", am.content.clone().unwrap_or_default()),
            ChatMessage::Tool(tm) => ("tool", tm.content.clone()),
        };
        format!("{}: {}", role, content)
    }).collect();

    let context_str = conversation_context.join("\n");
    let last_msg_content = last_user_message.map(|m| {
        match m {
            ChatMessage::User(um) => um.content.clone(),
            _ => String::new(),
        }
    }).unwrap_or_default();

    let api_messages = vec![
        ChatMessage::new_system(TITLE_GENERATION_SYSTEM_PROMPT),
        ChatMessage::new_user(format!(
            "Conversation:\n{}\n\nLatest message: {}",
            context_str, last_msg_content
        )),
    ];

    let body = json!({
        "model": "private-large",
        "stream": false,
        "messages": api_messages,
        "tools": [serde_json::from_str::<serde_json::Value>(GENERATE_TITLE_TOOL).unwrap()],
        "tool_choice": {
            "type": "function",
            "function": {
                "name": "generate_title"
            }
        }
    });

    let title = match call_llm_for_title(&state, body).await {
        Ok(t) => t,
        Err(_) => "New Chat".to_string(),
    };

    let cleaned_title = clean_title(&title);

    let _ = chat::update_chat_title(&state.db_pool, chat_id, &cleaned_title).await;

    Ok(Json(GenerateTitleResponse {
        title: cleaned_title,
    }))
}

async fn call_llm_for_title(
    state: &AppState,
    body: serde_json::Value,
) -> anyhow::Result<String> {
    let response = state
        .http
        .post(&state.litellm_url)
        .bearer_auth(&state.litellm_key)
        .json(&body)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let text = response.text().await?;
        anyhow::bail!("LiteLLM error {status}: {text}");
    }

    let chat_response: ChatResponse = response.json().await?;

    let choice = chat_response
        .choices
        .first()
        .ok_or_else(|| anyhow::anyhow!("No choices in response"))?;

    if let Some(tool_calls) = &choice.message.tool_calls {
        for tc in tool_calls {
            if tc.function.name == "generate_title" {
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)?;
                if let Some(title) = args.get("title").and_then(|v| v.as_str()) {
                    return Ok(title.to_string());
                }
            }
        }
    }

    anyhow::bail!("No title tool call found")
}

fn clean_title(title: &str) -> String {
    let mut cleaned = title.to_string();
    
    cleaned = cleaned.trim_matches(|c| c == '"' || c == '\'').to_string();
    
    cleaned = cleaned.replace("**", "");
    cleaned = cleaned.replace('*', "");
    cleaned = cleaned.replace('`', "");
    
    cleaned = cleaned.split('#').next().unwrap_or(&cleaned).to_string();
    
    if cleaned.starts_with('[') && cleaned.contains("](") {
        if let Some(start) = cleaned.find('[') {
            if let Some(end) = cleaned.find("](") {
                if let Some(close) = cleaned[end..].find(')') {
                    cleaned = cleaned[start+1..end+close].to_string();
                }
            }
        }
    }
    
    if cleaned.chars().count() > 50 {
        cleaned = cleaned.chars().take(50).collect();
    }
    
    if cleaned.trim().is_empty() {
        cleaned = "New Chat".to_string();
    }
    
    cleaned
}
