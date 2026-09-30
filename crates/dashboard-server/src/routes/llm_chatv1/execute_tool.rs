use axum::response::sse::Event;
use chrono::{Datelike, Utc};
use once_cell::sync::Lazy;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Mutex;
use uuid::Uuid;

use crate::AppState;
use crate::clickhouse;

use super::golden_queries;
use super::tools::get_database_schema_value;
use crate::db::live_dashboard;

static QUERY_RESULTS: Lazy<Mutex<HashMap<String, Value>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub async fn execute_tool(
    name: &str,
    arguments: &str,
    state: &AppState,
    project_id: Uuid,
    tx: &tokio::sync::mpsc::Sender<Result<Event, Infallible>>,
    tool_call_id: &str,
) -> Option<Value> {
    let args: Value = serde_json::from_str(arguments).unwrap_or(json!({}));

    match name {
        "get_current_datetime" => {
            let now = Utc::now();
            Some(json!({
                "current_datetime": now.format("%Y-%m-%d %H:%M:%S").to_string(),
                "utc_iso": now.to_rfc3339(),
                "date": now.format("%Y-%m-%d").to_string(),
                "time": now.format("%H:%M:%S").to_string(),
                "start_of_week": (now - chrono::Duration::days(now.weekday().num_days_from_monday() as i64)).format("%Y-%m-%d").to_string(),
                "start_of_month": now.format("%Y-%m-01").to_string(),
            }))
        }

        "execute_clickhouse_query" => {
            let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
            if query.is_empty() {
                return Some(json!({"error": "No query provided"}));
            }
            match clickhouse::execute_project_query(
                &state.clickhouse_url,
                project_id,
                &state.clickhouse_project_password,
                query,
            )
            .await
            {
                Ok(result) => {
                    if let Ok(mut map) = QUERY_RESULTS.lock() {
                        map.insert(tool_call_id.to_string(), result.clone());
                    }
                    Some(result)
                }
                Err(e) => Some(json!({"error": e.to_string()})),
            }
        }

        "get_database_schema" => {
            let table_name = args.get("table_name").and_then(|v| v.as_str());
            Some(get_database_schema_value(table_name))
        }

        "get_event_descriptions" => {
            let event_name = args.get("event_name").and_then(|v| v.as_str());
            match event_name {
                Some(name) => {
                    let query = format!(
                        "SELECT event_name, property, type, description FROM event_schema_catalog WHERE event_name = '{}'",
                        name
                    );
                    match clickhouse::execute_project_query(
                        &state.clickhouse_url,
                        project_id,
                        &state.clickhouse_project_password,
                        &query,
                    )
                    .await
                    {
                        Ok(result) => Some(result),
                        Err(e) => Some(json!({"error": e.to_string()})),
                    }
                }
                None => {
                    let query = "SELECT DISTINCT event_name FROM event_schema_catalog LIMIT 100";
                    match clickhouse::execute_project_query(
                        &state.clickhouse_url,
                        project_id,
                        &state.clickhouse_project_password,
                        query,
                    )
                    .await
                    {
                        Ok(result) => Some(result),
                        Err(e) => Some(json!({"error": e.to_string()})),
                    }
                }
            }
        }

        "list_golden_queries" => Some(golden_queries::list_golden_queries()),

        "list_live_dashboards" => {
            match live_dashboard::list_dashboards_summary(&state.db_pool, project_id).await {
                Ok(dashboards) => {
                    let list: Vec<Value> = dashboards
                        .into_iter()
                        .map(|(id, desc)| {
                            json!({
                                "id": id.to_string(),
                                "description": desc,
                            })
                        })
                        .collect();
                    Some(json!({ "live_dashboards": list }))
                }
                Err(e) => {
                    tracing::error!("Failed to list dashboards: {}", e);
                    Some(json!({ "error": "Failed to fetch dashboards" }))
                }
            }
        }

        "get_live_dashboard" => {
            let dashboard_id = args
                .get("dashboard_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if dashboard_id.is_empty() {
                return Some(json!({"error": "No dashboard_id provided"}));
            }
            match Uuid::parse_str(dashboard_id) {
                Ok(id) => match live_dashboard::get_dashboard(&state.db_pool, id).await {
                    Ok(Some(dashboard)) => Some(json!({
                        "id": dashboard.id.to_string(),
                        "title": dashboard.title,
                        "description": dashboard.description,
                        "query": dashboard.query,
                        "chart_config": dashboard.chart_config,
                        "last_ran_at": dashboard.last_ran_at,
                        "response": dashboard.response,
                        "creation_source": dashboard.creation_source,
                        "created_at": dashboard.created_at,
                        "updated_at": dashboard.updated_at,
                    })),
                    Ok(None) => Some(json!({"error": "Dashboard not found"})),
                    Err(e) => {
                        tracing::error!("Failed to get dashboard: {}", e);
                        Some(json!({"error": "Failed to fetch dashboard"}))
                    }
                },
                Err(_) => Some(json!({"error": "Invalid dashboard_id format"})),
            }
        }

        "fetch_golden_query" => {
            let identifier = args
                .get("identifier")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            Some(golden_queries::fetch_golden_query(identifier))
        }

        "report_failed_search" => {
            let user_intent = args
                .get("user_intent")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let attempted_keywords = args
                .get("attempted_keywords")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();
            tracing::info!(
                "Failed golden search - intent: {}, keywords: {:?}",
                user_intent,
                attempted_keywords
                    .iter()
                    .filter_map(|k| k.as_str())
                    .collect::<Vec<_>>()
            );
            Some(json!({"status": "reported"}))
        }

        _ => None,
    }
}
