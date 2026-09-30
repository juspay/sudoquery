use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// request



// response

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: AssistantMessage,
    pub finish_reason: Option<FinishReason>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub reasoning_content: Option<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: ToolCallType,
    pub function: FunctionCall,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolCallType {
    Function,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string, not a Value
}

#[derive(Debug, Default)]
pub struct ResponseAccumulator {
    pub id: String,
    pub model: String,
    pub finish_reason: Option<FinishReason>,
    pub role: String,
    pub content: String,
    pub reasoning_content: String,
    pub tool_calls: Vec<PartialToolCall>, // one per index
}

#[derive(Debug, Default)]
pub struct PartialToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String, // accumulated JSON string
}

impl ResponseAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn process_chunk(&mut self, chunk: &Value) {
        // top level fields — only in first chunk
        if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
            self.id = id.to_string();
        }
        if let Some(model) = chunk.get("model").and_then(|v| v.as_str()) {
            self.model = model.to_string();
        }

        let Some(choice) = chunk.get("choices").and_then(|c| c.get(0)) else {
            return;
        };

        // finish reason
        if let Some(reason) = choice.get("finish_reason").and_then(|r| r.as_str()) {
            self.finish_reason = match reason {
                "stop"           => Some(FinishReason::Stop),
                "length"         => Some(FinishReason::Length),
                "tool_calls"     => Some(FinishReason::ToolCalls),
                "content_filter" => Some(FinishReason::ContentFilter),
                _                => None,
            };
        }

        let Some(delta) = choice.get("delta") else {
            return;
        };

        // role — only in first chunk
        if let Some(role) = delta.get("role").and_then(|r| r.as_str()) {
            self.role = role.to_string();
        }

        // text content
        if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
            self.content.push_str(text);
        }

        // text content
        if let Some(text) = delta.get("reasoning_content").and_then(|c| c.as_str()) {
            self.reasoning_content.push_str(text);
        }

        // tool calls
        if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
            for tc in tool_calls {
                let index = tc.get("index")
                    .and_then(|i| i.as_u64())
                    .unwrap_or(0) as usize;

                // grow the vec if needed
                if self.tool_calls.len() <= index {
                    self.tool_calls.resize_with(index + 1, PartialToolCall::default);
                }

                let partial = &mut self.tool_calls[index];

                if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                    partial.id = id.to_string();
                }
                if let Some(name) = tc.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                {
                    partial.name = name.to_string();
                }
                if let Some(args) = tc.get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                {
                    partial.arguments.push_str(args);
                }
            }
        }
    }

    /// Call this after [DONE] to get the final response
    pub fn finish(self) -> ChatResponse {
        let tool_calls = if self.tool_calls.is_empty() {
            None
        } else {
            Some(
                self.tool_calls
                    .into_iter()
                    .enumerate()
                    .map(|(i, tc)| ToolCall {
                        id: tc.id,
                        kind: ToolCallType::Function,
                        function: FunctionCall {
                            name: tc.name,
                            arguments: tc.arguments,
                        },
                    })
                    .collect(),
            )
        };

        ChatResponse {
            id: self.id,
            model: self.model,
            choices: vec![Choice {
                index: 0,
                message: AssistantMessage {
                    role: self.role,
                    content: if self.content.is_empty() { None } else { Some(self.content) },
                    reasoning_content: if self.reasoning_content.is_empty() { None } else { Some(self.reasoning_content) },
                    tool_calls,
                },
                finish_reason: self.finish_reason,
            }],
            usage: None, // not available in streaming
        }
    }
}
