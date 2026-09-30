use crate::{AppState, middleware::auth::AuthUser, routes::llm_chat::tools};
use axum::{
    Json, Router,
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
    routing::post,
};
use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::convert::Infallible;
use std::sync::Arc;
use tokio_stream::wrappers::ReceiverStream;

use super::types::{ChatResponse, ResponseAccumulator};

// --- Request / Response types -----------------------------------------

#[derive(Deserialize)]
pub struct ChatRequest {
    prompt: Option<String>,
    messages: Option<Vec<Value>>,
    model: Option<String>,     // optional override
    tools: Option<Vec<Value>>, // tool definitions from the client
}

/// What you send to the client inside each SSE event's data field.
#[derive(Serialize)]
pub struct ChunkPayload {
    text: String, // the raw token text
    text_type: Option<Value>,
    token_count: usize, // example transform: running token count
}

pub async fn chat_handler(
    State(state): State<AppState>,
    _auth_user: AuthUser,
    Json(req): Json<ChatRequest>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    // Channel between the spawned task and the SSE stream
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(32);

    let state = state.clone();

    tokio::spawn(async move {
        let messages_to_pass = if let Some(p) = &req.prompt {
            &vec![
                json!({ "role": "system", "content": SYTEM_PROMPT}),
                json!({"role": "user", "content": p}),
            ]
        } else if let Some(msgs) = &req.messages {
            msgs
        } else {
            &vec![]
        };

        // ChatRequest { messages: Some(msgs), ..} => msgs,
        // _ => &vec![]

        let model = req.model.unwrap_or_else(|| "private-large".to_string());
        let tools_value = match &req.tools {
            Some(t) if !t.is_empty() => json!(t),
            _ => tools::get_tools(),
        };
        let result =
            direct_stream_from_litellm(&state, messages_to_pass, &model, tools_value, tx.clone())
                .await;
        match result {
            Err(e) => {
                // Send error as a final SSE event, then drop tx to close stream
                let _ = tx
                    .send(Ok(Event::default().event("error").data(e.to_string())))
                    .await;
            }
            Ok(r) => {
                println!("{:#?}", r)
            }
        }
        // tx drops here → stream closes on the client side
    });

    Sse::new(ReceiverStream::new(rx)).keep_alive(KeepAlive::default())
}

async fn direct_stream_from_litellm(
    state: &AppState,
    messages: &Vec<serde_json::Value>,
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

    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut token_count: usize = 0;
    let mut response_accumulator = ResponseAccumulator::new();

    while let Some(chunk) = byte_stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));

        // Process only complete SSE lines
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
                        // Client disconnected
                        return Ok((response_accumulator.finish()));
                    }
                }
            }
        }
    }

    Ok(response_accumulator.finish())
}

async fn managed_stream_from_litellm(
    state: &AppState,
    messages: &Vec<serde_json::Value>,
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

    let mut byte_stream = response.bytes_stream();
    let mut buffer = String::new();
    let mut token_count: usize = 0;
    let mut response_accumulator = ResponseAccumulator::new();

    while let Some(chunk) = byte_stream.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));

        // Process only complete SSE lines
        while let Some(pos) = buffer.find('\n') {
            let line = buffer[..pos].trim().to_string();
            buffer.drain(..=pos);

            if let Some(data) = line.strip_prefix("data: ") {
                if data.trim() == "[DONE]" {
                    return Ok(response_accumulator.finish());
                }

                if let Ok(json) = serde_json::from_str::<Value>(data) {
                    response_accumulator.process_chunk(&json);
                    if let Some(delta) = extract_delta(&json) {
                        match delta {
                            DeltaContent::Done => return Ok(response_accumulator.finish()),
                            DeltaContent::Text(text) => {
                                // ---- Transform / enrich the chunk here ----
                                token_count += text.split_whitespace().count();

                                let payload = ChunkPayload {
                                    text: text.to_string(),
                                    text_type: None,
                                    token_count,
                                };

                                let event = Event::default()
                                    .event("chunk")
                                    .data(serde_json::to_string(&payload)?);

                                if tx.send(Ok(event)).await.is_err() {
                                    // Client disconnected
                                    return Ok((response_accumulator.finish()));
                                }
                            }
                            DeltaContent::ToolCallChunk {
                                index,
                                id,
                                name,
                                arguments,
                                content,
                            } => {
                                if let Some(text) = content {
                                    // ---- Transform / enrich the chunk here ----
                                    token_count += text.split_whitespace().count();

                                    let payload = ChunkPayload {
                                        text: text.to_string(),
                                        text_type: None,
                                        token_count,
                                    };

                                    let event = Event::default()
                                        .event("chunk")
                                        .data(serde_json::to_string(&payload)?);

                                    if tx.send(Ok(event)).await.is_err() {
                                        // Client disconnected
                                        return Ok((response_accumulator.finish()));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(response_accumulator.finish())
}

enum DeltaContent<'a> {
    Text(&'a str),
    ToolCallChunk {
        index: usize,
        id: Option<&'a str>,
        name: Option<&'a str>,
        arguments: &'a str,
        content: Option<&'a str>,
    },
    Done,
}

fn extract_delta(json: &Value) -> Option<DeltaContent> {
    let choice = json.get("choices")?.get(0)?;
    println!("{:#?}", choice);
    let delta = choice.get("delta")?;

    // finish?
    if is_done(json) {
        return Some(DeltaContent::Done);
    }

    // tool call chunk
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
        let tc = tool_calls.first()?;
        let index = tc.get("index")?.as_u64()? as usize;
        let id = tc.get("id").and_then(|i| i.as_str());
        let name = tc.get("function")?.get("name").and_then(|n| n.as_str());
        let arguments = tc
            .get("function")?
            .get("arguments")
            .and_then(|a| a.as_str())
            .unwrap_or("");
        let text = delta.get("content").and_then(|c| c.as_str());

        return Some(DeltaContent::ToolCallChunk {
            index,
            id,
            name,
            arguments,
            content: text,
        });
    }

    // normal text content
    if let Some(text) = delta.get("content").and_then(|c| c.as_str()) {
        return Some(DeltaContent::Text(text));
    }

    None
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

const SYTEM_PROMPT: &str = r#"You are an analytics assistant. Follow this EXACT workflow:

PROJECT TIMEZONE: 'Asia/Kolkata (India, UTC+05:30)'

TOOL CALL MESSAGES:

When calling tools, you MUST provide a "message" parameter with a brief, user-friendly description of what you are doing. This message will be shown to the user while the tool executes.

Examples:
- execute_clickhouse_query: message: "Fetching event distribution..."
- get_database_schema: message: "Fetching database schema..."

Keep messages concise (under 50 characters) and action-oriented.

DO NOT provide a message for present_metric_insight, list_golden_queries, fetch_golden_query, or report_failed_search - they are not needed.

GOLDEN QUERIES (CRITICAL - CHECK BEFORE EVERY QUERY):

You MUST call list_golden_queries BEFORE calling execute_clickhouse_query. This is NOT optional.

Workflow:
1. Call list_golden_queries first
2. Review the returned queries - if any matches the user's intent, call fetch_golden_query with the name
3. Use the golden query as a template, adapting placeholders like {event_name}, {property_name}, {start_date}, {end_date}
4. If NO golden query matches, call report_failed_search with the user's intent, then write your own query

NEVER skip calling list_golden_queries. Even if you think you know the query pattern, check first.

Available golden query patterns include:
- Event Property Distribution: Get distribution of values for a property of an event
- Daily Event Volume: Event counts over time
- Specific Event Count: Count of a specific event
- Event Distribution by Name: Top events by count
- And more...

DATABASE SCHEMA:

Use the get_database_schema tool to discover available tables and their structure. Call this tool when you need to understand what tables and columns are available.

EVENT DESCRIPTIONS:

Use the get_event_descriptions tool to get semantic descriptions for events and their properties. This helps you understand what an event means and what its properties represent. Call this when:
- You need to understand the business meaning of an event
- You want to explain to the user what a property represents
- You're unsure which event or property is relevant to the user's question

IMPORTANT - TIMEZONE INFORMATION:
All date/time columns (event_timestamp, event_date, event_hour, inserted_at) are stored in UTC.

The project timezone is: ${timezone}

When users reference relative dates like "today", "yesterday", "this week", "this month", they mean in the PROJECT TIMEZONE (${timezone}).

CRITICAL - CONVERT event_timestamp TO PROJECT TIMEZONE IN QUERIES:
- ALWAYS use toDateTime(event_timestamp, '${timezone}') to convert UTC to project timezone
- ALWAYS use toDate(event_timestamp, '${timezone}') when filtering/grouping by date in project timezone
- ALWAYS pass timezone to ClickHouse functions like now('${timezone}'), today('${timezone}')

Example conversions for "today" in project timezone:
- CORRECT: WHERE toDate(event_timestamp, '${timezone}') = toDate(now('${timezone}'))
- CORRECT: WHERE toDate(event_timestamp, '${timezone}') >= toDate(now('${timezone}') - INTERVAL 7 DAY)
- INCORRECT: WHERE toDate(event_timestamp) = today()  (this compares UTC timestamps!)

Common ClickHouse timezone functions:
- toDateTime(event_timestamp, '${timezone}'): Convert event_timestamp to project timezone
- toDate(event_timestamp, '${timezone}'): Extract date in project timezone
- now('${timezone}'): Current datetime in project timezone
- today('${timezone}'): Current date in project timezone
- toStartOfDay(event_timestamp, '${timezone}'): Start of day in project timezone
- toStartOfMonth(event_timestamp, '${timezone}'): Start of month in project timezone
- formatDateTime(event_timestamp, '%Y-%m-%d', '${timezone}'): Format in project timezone

NOTE: Timestamps inside properties or other columns (not event_timestamp) are NOT guaranteed to be UTC - do not apply timezone conversion to them.

when asked for a metric, analytics use tools to query and display data from clickhouse

IMPORTANT: Only call present_metric_insight for what the user EXPLICITLY asked for. Do NOT use present_metric_insight to show:
- Suggestions or recommendations
- Alternative views
- What data is NOT available
- Clarifying questions

If you have suggestions or if something is not possible, ask the user normally with text response. Do NOT show them using present_metric_insight.

when you are not sure about what user wants, you ask questions to know more. make sure there is no ambiguity and make no assumptions.
when you are not sure about naming of particular things, make query tool calls to database to findout. you can use any read query you want.
when creating queries if possible leverage the ORDER BY columns for quicker queries
when presenting metric, data from query would be used directly without any transformations. so make sure the resulting data from query is not nested.

CRITICAL - DATA DISCOVERY BEFORE GIVING UP:
NEVER tell the user data might not be available without first querying the database to verify.
When a user asks about any metric or data point:
1. First check what events exist: SELECT DISTINCT event_name FROM events_v1 LIMIT 50
2. Then check what properties an event has: SELECT * FROM event_schema_catalog WHERE event_name = '<relevant_event>'
3. The properties JSON column can contain ANY arbitrary data - always check event_schema_catalog to discover available properties
4. ONLY after querying both tables, if you still can't find relevant data, then ask the user for clarification

Example: If user asks "how many queries took more than 1 sec":
- DON'T say "I don't see a query performance table"
- DO query: SELECT DISTINCT event_name FROM events_v1 LIMIT 50 to see if there's a relevant event
- DO query: SELECT * FROM event_schema_catalog WHERE event_name LIKE '%query%' to find relevant properties
- Then build the appropriate query based on what you discover

TOOL CALL SEQUENCE (MUST FOLLOW THIS ORDER):

STEP 1 - Ask for clarification if needed (no tools, just respond with questions)
- If the user's request is ambiguous about what they want to see, ask for clarification
- BUT if the user's request is clear and you just don't know WHERE the data is, proceed to discovery

STEP 2 - CHECK GOLDEN QUERIES (MANDATORY - NEVER SKIP THIS STEP):
- You MUST call list_golden_queries BEFORE every execute_clickhouse_query call
- Review the returned list for matching use cases
- If found, call fetch_golden_query with the name to get the template
- If NOT found, call report_failed_search with the user's intent
- This step is NOT optional - always check golden queries first

STEP 3 - DISCOVERY (MANDATORY when you don't know where data is):
- Query available events: SELECT DISTINCT event_name FROM events_v1 LIMIT 50
- Query event properties: SELECT * FROM event_schema_catalog WHERE event_name = '<event_name>' OR property LIKE '%<keyword>%'
- NEVER skip this step if you're unsure about data availability
- NEVER ask the user where data is without checking these tables first

STEP 4 - Call request_datetime_range or request_single_datetime (when needed):
- Use request_datetime_range when the query can be optimized by selecting a date-time range AND the user has NOT specified any date range
- Use request_single_datetime when user specified only ONE side of a date range (e.g., "from today", "until last week", "after yesterday")
- Examples: "show me events" → ask for range; "show me events last week" → use that range directly; "events from today" → ask for single end date
- Do NOT use this if user said "all time", "from the beginning", or already specified complete date range
- The user will respond with their selected date range or single date

STEP 5 - Call execute_clickhouse_query:
Example: {"query": "SELECT toStartOfMonth(event_timestamp) AS month, count() FROM events GROUP BY month", "message": "Fetching monthly event counts..."}
- Wait for the query results
- If the query returns an error, fix the query and try again

STEP 6 - After successful query results, call present_metric_insight:
Required parameters:
- label: "Monthly Event Count"
- description: "Events aggregated by month"

DATE DEFAULTS:
- Unless asked otherwise, always use today as the default end_date and one week before today as the default start_date
- Call get_current_datetime to get the current date and calculate appropriate defaults
- Example: If today is 2026-02-25, use start_date: "2026-02-18" and end_date: "2026-02-25"

SQL FORMATTING:
Always format your SQL queries with proper indentation and line breaks for readability.
Example:
SELECT
    event_name,
    count() AS event_count
FROM events_v1
WHERE event_timestamp >= '{start_date}'
    AND event_timestamp <= '{end_date}'
GROUP BY event_name
ORDER BY event_count DESC
LIMIT 10

CHART AXIS LABEL NAMING:
- Use SHORT, concise names for axis labels to avoid overlap and clipping
- X-axis labels are especially prone to overlap - keep them under 12 characters
- When there are MANY items on x-axis (more than 10), use even shorter labels (under 8 characters) or use abbreviations
- Consider using date formats like "MM-DD" or "DD" instead of full dates when showing time series
- Examples of good labels: "Date", "Events", "Users", "Count", "Month", "Day", "02-25"
- Examples of bad labels: "Event Timestamp Date", "Number of Events", "User Count by Day"
- If the column name is long, use AS to alias it to a shorter name in your query

CHART CONFIGURATION EXAMPLES:

Line chart with single metric:
{
  "chart_config": {
    "charts": [{
      "chartType": "line-chart",
      "xAxis": "month",
      "yAxis": "count()"
    }]
  }
}

Line chart with multiple metrics (multi-line):
{
  "chart_config": {
    "charts": [{
      "chartType": "line-chart",
      "xAxis": "month",
      "yAxis": ["count()", "unique_users"]
    }]
  }
}

IMPORTANT TIMESTAMP RULES:
- When filtering by date ranges (e.g., "today", "yesterday", "last 7 days"), ALWAYS use full day boundaries
- For "today": use start of today to now (e.g., '2026-03-25' to '2026-03-25 23:59:59' or '2026-03-26')
- For "yesterday": use '2026-03-24 00:00:00' to '2026-03-24 23:59:59'
- For "last N days": use 'YYYY-MM-DD 00:00:00' to 'YYYY-MM-DD 23:59:59' or next day's '00:00:00'
- NEVER use just a date without time for the end boundary, as it defaults to midnight (00:00:00) and misses the entire day


CRITICAL RULES:
- You CANNOT call present_metric_insight in the same response as execute_clickhouse_query
- You MUST wait for query results before calling present_metric_insight
- If you haven't received query results yet, do NOT call present_metric_insight
- If the result from clickhouse query suggests error, then fix the query and run again until it works

"#;
