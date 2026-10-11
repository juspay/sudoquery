use crate::{
    AppState,
    db::{ChatType, chat, message},
    middleware::{ProjectAccess, auth::AuthUser},
    routes::llm_chatv1::constants::get_system_prompt,
};
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use canonical_event::ProjectId;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::convert::Infallible;
use tokio_stream::wrappers::ReceiverStream;
use uuid::Uuid;

use super::execute_tool;
use super::tools::get_tools;

use super::types::{ChatMessage, ChatResponse, MessageRole, ResponseAccumulator};

const CLIENT_HANDLED_TOOLS: &[&str] = &["request_datetime_range", "request_single_datetime"];

#[derive(Deserialize, Serialize)]
pub struct ChatRequest {
    prompt: String,
    chat_id: Option<Uuid>,
    message_id: Option<Uuid>, // If provided, edits this message and truncates conversation after it
    chat_type: ChatType,
}

#[derive(Debug)]
pub enum ChatHandlerError {
    Database(String),
    NotFound(String),
    Forbidden(String),
}

impl axum::response::IntoResponse for ChatHandlerError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match self {
            ChatHandlerError::Database(msg) => {
                tracing::error!("Database error: {}", msg);
                (StatusCode::INTERNAL_SERVER_ERROR, msg)
            }
            ChatHandlerError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            ChatHandlerError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

pub async fn chat_handler_v1(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<ChatRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ChatHandlerError> {
    let chat_id: Uuid = match req.chat_id {
        Some(existing_chat_id) => {
            match chat::get_chat_by_id(&state.db_pool, existing_chat_id).await {
                Ok(Some(chat)) => {
                    if chat.project_id != project.id {
                        return Err(ChatHandlerError::Forbidden(
                            "Chat not found in this project".to_string(),
                        ));
                    }
                    if chat.chat_type != req.chat_type {
                        return Err(ChatHandlerError::Forbidden("ChatType mismatch".to_string()));
                    }
                    existing_chat_id
                }
                Ok(None) => {
                    return Err(ChatHandlerError::NotFound("Chat not found".to_string()));
                }
                Err(e) => {
                    return Err(ChatHandlerError::Database(e.to_string()));
                }
            }
        }
        None => {
            match chat::create_chat(
                &state.db_pool,
                &project.id,
                _auth_user.user.id,
                "New Chat",
                &req.chat_type,
            )
            .await
            {
                Ok(new_chat) => new_chat.id,
                Err(e) => {
                    return Err(ChatHandlerError::Database(format!(
                        "Failed to create chat: {}",
                        e
                    )));
                }
            }
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    tx.send(Ok(Event::default()
        .event("chat_id")
        .data(chat_id.to_string())))
        .await
        .unwrap();

    // Handle edit mode: if message_id is provided, update that message and truncate conversation
    let messages_so_far = if let Some(edit_message_id) = req.message_id {
        // Verify the message exists and belongs to this chat
        let msg = match message::get_message_by_id(&state.db_pool, edit_message_id).await {
            Ok(Some(m)) => m,
            Ok(None) => {
                return Err(ChatHandlerError::NotFound("Message not found".to_string()));
            }
            Err(e) => {
                return Err(ChatHandlerError::Database(e.to_string()));
            }
        };

        if msg.chat_id != chat_id {
            return Err(ChatHandlerError::Forbidden(
                "Message does not belong to this chat".to_string(),
            ));
        }

        if msg.role != "user" {
            return Err(ChatHandlerError::Forbidden(
                "Can only edit user messages".to_string(),
            ));
        }

        // Update the message content
        let updated_message = json!({
            "content": req.prompt,
            "role": "user"
        });

        println!("edit_message_id is {}", edit_message_id);
        if let Err(e) =
            message::update_message_content(&state.db_pool, edit_message_id, &updated_message).await
        {
            tracing::error!("Failed to update message: {}", e);
            return Err(ChatHandlerError::Database(
                "Failed to update message".to_string(),
            ));
        }

        // Delete all messages after this message_id
        if let Err(e) =
            message::delete_messages_after(&state.db_pool, chat_id, edit_message_id).await
        {
            tracing::error!("Failed to delete messages: {}", e);
            return Err(ChatHandlerError::Database(
                "Failed to delete subsequent messages".to_string(),
            ));
        }

        // Fetch messages up to and including the edited message
        message::list_messages_by_chat(&state.db_pool, chat_id)
            .await
            .unwrap()
    } else {
        // Normal flow: create new message
        if let Err(e) = message::create_message(
            &state.db_pool,
            chat_id,
            &json!({"content": req.prompt, "role": "user"}),
            "user",
        )
        .await
        {
            tracing::error!("Failed to save message: {}", e);
            return Err(ChatHandlerError::Database(
                "Failed to save message".to_string(),
            ));
        }

        message::list_messages_by_chat(&state.db_pool, chat_id)
            .await
            .unwrap()
    };

    let state = state.clone();
    let project_id = project.id;

    let mut messages: Vec<ChatMessage> = messages_so_far
        .iter()
        .map(|m| {
            let msg = m.message.as_object().unwrap();
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

            match role {
                "system" => ChatMessage::new_system(content),
                "user" => ChatMessage::new_user(content),
                "assistant" => {
                    let tool_calls: Option<Vec<super::types::ToolCall>> = msg
                        .get("tool_calls")
                        .and_then(|v| serde_json::from_value(v.clone()).ok());
                    let reasoning_content = msg
                        .get("reasoning_content")
                        .and_then(|v| v.as_str())
                        .map(String::from);
                    let content_opt = if content.is_empty() {
                        None
                    } else {
                        Some(content.to_string())
                    };
                    ChatMessage::new_assistant_with_tool_calls(
                        content_opt,
                        tool_calls,
                        reasoning_content,
                    )
                }
                _ => ChatMessage::new_user(content),
            }
        })
        .collect();

    tokio::spawn(async move {
        messages.insert(
            0,
            ChatMessage::new_system(get_system_prompt(&req.chat_type)),
        );
        talk_to_llm(messages, &state, tx, chat_id, project_id, &req.chat_type).await;
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

// clients respond to tool call by passing chat_id, tool_call_id and value
// check the tool_call_id matches with last tool call in the conversation
// if matches, store it in messages_to_store and make llm request with this response
// streaming is asusual
// store the messages in terminal state

#[derive(Deserialize)]
pub struct ClientToolCallRequest {
    pub chat_id: Uuid,
    pub tool_call_id: String,
    pub value: Value,
}

pub async fn client_tool_call_response_v1(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<ClientToolCallRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, ChatHandlerError> {
    let chat_id = req.chat_id;

    let chat = match chat::get_chat_by_id(&state.db_pool, chat_id).await {
        Ok(Some(c)) => c,
        Ok(None) => {
            return Err(ChatHandlerError::NotFound("Chat not found".to_string()));
        }
        Err(e) => {
            return Err(ChatHandlerError::Database(e.to_string()));
        }
    };

    if chat.project_id != project.id {
        return Err(ChatHandlerError::Forbidden(
            "Chat not found in this project".to_string(),
        ));
    }

    let messages_so_far = match message::list_messages_by_chat(&state.db_pool, chat_id).await {
        Ok(msgs) => msgs,
        Err(e) => {
            return Err(ChatHandlerError::Database(e.to_string()));
        }
    };

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    let state = state.clone();
    let project_id = project.id;
    let tool_call_id = req.tool_call_id;
    let value = req.value;

    tokio::spawn(async move {
        let mut messages: Vec<ChatMessage> = messages_so_far
            .iter()
            .map(|m| {
                let msg = m.message.as_object().unwrap();
                let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("user");
                let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");

                match role {
                    "system" => ChatMessage::new_system(content),
                    "user" => ChatMessage::new_user(content),
                    "assistant" => {
                        let tool_calls: Option<Vec<super::types::ToolCall>> = msg
                            .get("tool_calls")
                            .and_then(|v| serde_json::from_value(v.clone()).ok());
                        let reasoning_content = msg
                            .get("reasoning_content")
                            .and_then(|v| v.as_str())
                            .map(String::from);
                        let content_opt = if content.is_empty() {
                            None
                        } else {
                            Some(content.to_string())
                        };
                        ChatMessage::new_assistant_with_tool_calls(
                            content_opt,
                            tool_calls,
                            reasoning_content,
                        )
                    }
                    _ => ChatMessage::new_user(content),
                }
            })
            .collect();

        messages.insert(
            0,
            ChatMessage::new_system(get_system_prompt(&chat.chat_type)),
        );

        let last_assistant_has_tool_call = messages
            .last()
            .and_then(|m| {
                if let ChatMessage::Assistant(assistant_msg) = m {
                    assistant_msg
                        .tool_calls
                        .as_ref()
                        .map(|tool_calls| tool_calls.iter().any(|tc| tc.id == tool_call_id))
                } else {
                    None
                }
            })
            .unwrap_or(false);

        println!("Last message: {:?}", messages.last());
        println!(
            "Last assistant has tool call: {}",
            last_assistant_has_tool_call
        );

        if last_assistant_has_tool_call {
            let tool_response_msg = ChatMessage::new_tool(
                &tool_call_id,
                serde_json::to_string(&value).unwrap_or_default(),
            );
            let tool_response_value = serde_json::to_value(&tool_response_msg).unwrap_or_default();
            messages.push(tool_response_msg);
            // write message to database with role tool
            message::create_message(&state.db_pool, chat_id, &tool_response_value, "tool")
                .await
                .unwrap();
            talk_to_llm(
                messages,
                &state,
                tx.clone(),
                chat_id,
                project_id,
                &chat.chat_type,
            )
            .await;
        } else {
            let error_json =
                json!({"error": "No pending tool call found with matching ID"}).to_string();
            let _ = tx
                .send(Ok(Event::default().event("error").data(error_json)))
                .await;
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default()))
}

#[derive(Serialize)]
pub struct MessagesResponse {
    pub messages: Vec<MessageItem>,
}

#[derive(Serialize)]
pub struct MessageItem {
    pub id: String,
    pub chat_id: String,
    pub message: Value,
    pub created_at: String,
}

pub async fn get_chat_messages(
    State(state): State<AppState>,
    Path(chat_id): Path<String>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> Result<Json<MessagesResponse>, StatusCode> {
    let chat_id = Uuid::parse_str(&chat_id).map_err(|_| StatusCode::BAD_REQUEST)?;

    let chat = chat::get_chat_by_id(&state.db_pool, chat_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if chat.project_id != project.id {
        return Err(StatusCode::FORBIDDEN);
    }

    let messages = message::list_messages_by_chat(&state.db_pool, chat_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let message_items: Vec<MessageItem> = messages
        .into_iter()
        .map(|m| MessageItem {
            id: m.id.to_string(),
            chat_id: m.chat_id.to_string(),
            message: m.message,
            created_at: m.created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(MessagesResponse {
        messages: message_items,
    }))
}

async fn talk_to_llm(
    mut messages: Vec<ChatMessage>,
    state: &AppState,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    chat_id: Uuid,
    project_id: ProjectId,
    chat_type: &ChatType,
) {
    let model = "private-large".to_string();

    loop {
        let result =
            call_litellm(&state, &messages, &model, get_tools(chat_type), tx.clone()).await;
        match result {
            Err(e) => {
                let error_json = json!({"error": e.to_string()}).to_string();
                let _ = tx
                    .send(Ok(Event::default().event("error").data(error_json)))
                    .await;
                return;
            }
            Ok(response) => {
                let Some(choice) = response.choices.first() else {
                    return;
                };

                let assistant_msg = choice.message.clone();
                let msg_value = serde_json::to_value(&assistant_msg).unwrap_or_default();
                messages.push(ChatMessage::Assistant(assistant_msg));
                // add message to database with role assistant
                let saved_msg =
                    match message::create_message(&state.db_pool, chat_id, &msg_value, "assistant")
                        .await
                    {
                        Ok(msg) => msg,
                        Err(e) => {
                            tracing::error!("Failed to save assistant message: {}", e);
                            let error_json = json!({"error": "Failed to save message"}).to_string();
                            let _ = tx
                                .send(Ok(Event::default().event("error").data(error_json)))
                                .await;
                            return;
                        }
                    };

                // Send message_end with the message_id
                let message_end_json = json!({"message_id": saved_msg.id.to_string()}).to_string();
                let _ = tx
                    .send(Ok(Event::default()
                        .event("message_end")
                        .data(message_end_json)))
                    .await;

                let Some(tool_calls) = &choice.message.tool_calls else {
                    return;
                };

                for tc in tool_calls {
                    let tool_name = &tc.function.name;

                    let tool_call_id = &tc.id;
                    if let Some(tool_result) = execute_tool::execute_tool(
                        tool_name,
                        &tc.function.arguments,
                        state,
                        &project_id,
                        &tx,
                        tool_call_id,
                    )
                    .await
                    {
                        let tool_msg = ChatMessage::new_tool(
                            tool_call_id,
                            serde_json::to_string(&tool_result).unwrap_or_default(),
                        );
                        let tool_msg_value = serde_json::to_value(&tool_msg).unwrap_or_default();
                        messages.push(tool_msg.clone());
                        let _ = tx
                            .send(Ok(Event::default()
                                .event("complete_message")
                                .data(serde_json::to_string(&tool_msg).unwrap())))
                            .await;
                        // add tool message to database
                        let saved_msg = match message::create_message(
                            &state.db_pool,
                            chat_id,
                            &tool_msg_value,
                            "tool",
                        )
                        .await
                        {
                            Ok(msg) => msg,
                            Err(e) => {
                                tracing::error!("Failed to save tool message: {}", e);
                                let error_json =
                                    json!({"error": "Failed to save message"}).to_string();
                                let _ = tx
                                    .send(Ok(Event::default().event("error").data(error_json)))
                                    .await;
                                return;
                            }
                        };
                        let message_end_json =
                            json!({"message_id": saved_msg.id.to_string()}).to_string();
                        let _ = tx
                            .send(Ok(Event::default()
                                .event("message_end")
                                .data(message_end_json)))
                            .await;
                    } else {
                        return;
                    }
                }
            }
        }
    }
}

pub async fn call_litellm(
    state: &AppState,
    messages: &[ChatMessage],
    model: &str,
    tools: Value,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> anyhow::Result<ChatResponse> {
    let body = json!({
        "model": model,
        "stream": true,
        "messages": messages,
        "tools": tools
    });

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

    stream_response(response, tx).await
}

async fn stream_response(
    response: reqwest::Response,
    tx: tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
) -> anyhow::Result<ChatResponse> {
    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut response_accumulator = ResponseAccumulator::new();

    while let Some(chunk) = byte_stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));

        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer.drain(..=pos);

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    return Ok(response_accumulator.finish());
                }

                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    response_accumulator.process_chunk(&json);

                    let event = Event::default()
                        .event("chunk")
                        .data(serde_json::to_string(&json)?);
                    if tx.send(Ok(event)).await.is_err() || is_done(&json) {
                        return Ok(response_accumulator.finish());
                    }
                }
            }
        }
    }

    Ok(response_accumulator.finish())
}

fn is_done(json: &Value) -> bool {
    if let Some(choice) = json.get("choices").and_then(|c| c.get(0)) {
        return choice
            .get("finish_reason")
            .and_then(|r| r.as_str())
            .is_some();
    }
    false
}
