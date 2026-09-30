use serde_json::json;

use crate::db::ChatType;

pub fn get_tools(chat_type: &ChatType) -> serde_json::Value {
    let GET_CURRENT_DATETIME_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "get_current_datetime",
            "description": "Get the current date and time. Use this before creating queries with date filters to ensure you use appropriate date ranges. Returns current date, time, and useful date values for queries.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
        },
    });

    let REQUEST_DATETIME_RANGE_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "request_datetime_range",
            "description": "Request the user to select a date-time range for the query. Use this when the user asks for metrics that can be optimized by selecting a date-time range, and they have not explicitly specified a range. This will open a UI for the user to select start and end dates. Do NOT use this if the user already specified a date range or asked for all-time data.",
            "parameters": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A brief message explaining why the date range is needed (e.g., \"Please select a date range for the event analysis\")",
                    },
                },
                "required": ["message"],
            },
        },
    });

    let REQUEST_SINGLE_DATETIME_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "request_single_datetime",
            "description": "Request the user to select a single date-time point. Use this when only one side of a date range is specified (e.g., user said \"from today\" or \"until yesterday\" but didn't specify the other end). This will open a UI for the user to select a date. Do NOT use this if the user specified a complete date range or asked for all-time data.",
            "parameters": {
                "type": "object",
                "properties": {
                    "message": {
                        "type": "string",
                        "description": "A brief message explaining what date-time is needed (e.g., \"Please select an end date\")",
                    },
                },
                "required": ["message"],
            },
        },
    });

    let CLICKHOUSE_QUERY_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "execute_clickhouse_query",
            "description": "Execute a ClickHouse SQL query to fetch analytics data. Returns raw query results.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The ClickHouse SQL query to execute",
                    },
                    "message": {
                        "type": "string",
                        "description": "A brief, user-friendly message (e.g., \"Fetching event distribution...\")",
                    },
                },
                "required": ["query", "message"],
            },
        },
    });

    let PRESENT_METRIC_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "present_metric_insight",
            "description": "Present query results as a visualizable metric insight with a label. Call this after execute_clickhouse_query to display the results as a chart.",
            "parameters": {
                "type": "object",
                "properties": {
                    "label": {
                        "type": "string",
                        "description": "Descriptive label for this metric (e.g., \"Monthly Event Volume\", \"User Conversion Rate\")",
                    },
                    "description": {
                        "type": "string",
                        "description": "Brief description of what this metric represents",
                    },
                    "chart_config": {
                        "type": "object",
                        "description": "Chart configuration. Specify chartType and the required axis fields for that chart type.",
                        "properties": {
                            "charts": {
                                "type": "array",
                                "description": "Array of chart configurations. The first chart will be shown by default.",
                                "items": {
                                    "type": "object",
                                    "properties": {
                                        "id": {
                                            "type": "string",
                                            "description": "Optional unique identifier for this chart.",
                                        },
                                        "chartType": {
                                            "type": "string",
                                            "enum": ["bar-chart", "line-chart", "pie-chart", "sankey-chart", "funnel-chart", "segmented-matrix"],
                                            "description": "Type of chart to render. Each type requires specific axis fields.",
                                        },
                                        "xAxis": {
                                            "type": "string",
                                            "description": "Column for x-axis. Required for bar-chart and line-chart. For segmented-matrix, this is the row dimension.",
                                        },
                                        "yAxis": {
                                            "type": "string",
                                            "description": "Column for y-axis. Required for bar-chart and line-chart. For line charts, can be an array of column names for multiple lines. For segmented-matrix, this is the column dimension.",
                                        },
                                        "labelAxis": {
                                            "type": "string",
                                            "description": "Column for labels. Required for pie-chart and funnel-chart.",
                                        },
                                        "valueAxis": {
                                            "type": "string",
                                            "description": "Column for values. Required for pie-chart, sankey-chart, funnel-chart, and segmented-matrix.",
                                        },
                                    },
                                },
                            },
                        },
                    },
                },
                "required": ["label", "description"],
            },
        },
    });

    let GET_DATABASE_SCHEMA_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "get_database_schema",
            "description": "Get the database schema information. Returns table names, column names, and data types. Use this to discover available tables and their structure before writing queries.",
            "parameters": {
                "type": "object",
                "properties": {
                    "table_name": {
                        "type": "string",
                        "description": "Optional: specific table name to get schema for. If not provided, returns schema for all tables.",
                    },
                    "message": {
                        "type": "string",
                        "description": "A brief, user-friendly message (e.g., \"Fetching database schema...\")",
                    },
                },
                "required": ["message"],
            },
        },
    });

    let LIST_GOLDEN_QUERIES_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "list_golden_queries",
            "description": "List all available golden queries by name. Golden queries are pre-defined optimal query patterns for common use cases. Call this BEFORE writing your own query to check if a similar pattern already exists. Returns query names, descriptions, use cases, and tags.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
        },
    });

    let FETCH_GOLDEN_QUERY_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "fetch_golden_query",
            "description": "Fetch a specific golden query by name or ID. Returns the full query pattern, chart configuration suggestions, and implementation details. Use this after list_golden_queries to get the exact query pattern for your use case.",
            "parameters": {
                "type": "object",
                "properties": {
                    "identifier": {
                        "type": "string",
                        "description": "The name or ID of the golden query to fetch (e.g., \"Daily Event Volume\" or \"gq-001\")",
                    },
                },
                "required": ["identifier"],
            },
        },
    });

    let REPORT_FAILED_SEARCH_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "report_failed_search",
            "description": "Report when no suitable golden query was found for the user's use case. This helps improve the golden query library. Call this after searching golden queries and not finding a matching pattern.",
            "parameters": {
                "type": "object",
                "properties": {
                    "user_intent": {
                        "type": "string",
                        "description": "What the user is trying to accomplish (e.g., \"compare conversion rates between platforms\")",
                    },
                    "attempted_keywords": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Keywords you used to search for golden queries",
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why no golden query was suitable (optional)",
                    },
                },
                "required": ["user_intent", "attempted_keywords"],
            },
        },
    });

    let GET_EVENT_DESCRIPTIONS_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "get_event_descriptions",
            "description": "Get descriptions for events and their properties. Use this to understand what an event means and what its properties represent. Call this when you need context about event semantics before writing queries.",
            "parameters": {
                "type": "object",
                "properties": {
                    "event_name": {
                        "type": "string",
                        "description": "Optional: specific event name to get descriptions for. If not provided, returns descriptions for all events.",
                    },
                    "message": {
                        "type": "string",
                        "description": "A brief, user-friendly message (e.g., \"Fetching event descriptions...\")",
                    },
                },
                "required": ["message"],
            },
        },
    });

    let LIST_LIVE_DASHBOARDS_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "list_live_dashboards",
            "description": "List all available live dashboards for the project. Returns dashboard IDs and their descriptions. Use this to discover what dashboards are available before fetching specific ones.",
            "parameters": {
                "type": "object",
                "properties": {},
                "required": [],
            },
        },
    });

    let GET_LIVE_DASHBOARD_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "get_live_dashboard",
            "description": "Get a specific live dashboard by its ID. Returns the full dashboard including query, description, chart config, and last run results. Use this after list_live_dashboards to fetch a specific dashboard.",
            "parameters": {
                "type": "object",
                "properties": {
                    "dashboard_id": {
                        "type": "string",
                        "description": "The UUID of the dashboard to fetch",
                    },
                },
                "required": ["dashboard_id"],
            },
        },
    });

    let TEST_RUN_LIVE_DASHBOARD_TOOL: serde_json::Value = json!({
        "type": "function",
        "function": {
            "name": "test_run_live_dashboard",
            "description": "Test run a live dashboard configuration. Executes the query and returns results with chart visualization preview. Use this to validate the dashboard before saving. Shows the user what the dashboard will look like.",
            "parameters": {
                "type": "object",
                "properties": {
                    "query_tool_call_id": {
                        "type": "string",
                        "description": "Previous query tool call id. Query from this toolcall will be used to run and show data in dashboard.",
                    },
                    "label":{
                        "Title": "string",
                        "description": "Title for dashboard",
                    },
                    "description": {
                        "type": "string",
                        "description": "Description of what this dashboard shows",
                    },
                    "chart_config": {
                        "type": "array",
                        "description": "Array of chart configurations. The first chart will be shown by default.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "chartType": {
                                    "type": "string",
                                    "enum": ["bar-chart", "line-chart", "pie-chart", "funnel-chart", "segmented-matrix"],
                                    "description": "Type of chart to render",
                                },
                                "xAxis": {
                                    "type": "string",
                                    "description": "Column name for x-axis (for bar/line charts)",
                                },
                                "yAxis": {
                                    "type": "string",
                                    "description": "Column name for y-axis (for bar/line charts)",
                                },
                                "labelAxis": {
                                    "type": "string",
                                    "description": "Column name for labels (for pie/funnel charts)",
                                },
                                "valueAxis": {
                                    "type": "string",
                                    "description": "Column name for values (for pie/funnel charts)",
                                },
                            },
                            "required": ["chartType"],
                        },
                    },
                },
                "required": ["query_tool_call_id", "description", "label", "chart_config"],
            },
        },
    });

    let tools = match chat_type {
        ChatType::General => vec![
            GET_CURRENT_DATETIME_TOOL,
            REQUEST_DATETIME_RANGE_TOOL,
            REQUEST_SINGLE_DATETIME_TOOL,
            GET_DATABASE_SCHEMA_TOOL,
            GET_EVENT_DESCRIPTIONS_TOOL,
            LIST_LIVE_DASHBOARDS_TOOL,
            GET_LIVE_DASHBOARD_TOOL,
            LIST_GOLDEN_QUERIES_TOOL,
            FETCH_GOLDEN_QUERY_TOOL,
            REPORT_FAILED_SEARCH_TOOL,
            CLICKHOUSE_QUERY_TOOL,
            PRESENT_METRIC_TOOL,
        ],
        ChatType::CreateDashboard => vec![
            GET_CURRENT_DATETIME_TOOL,
            GET_DATABASE_SCHEMA_TOOL,
            GET_EVENT_DESCRIPTIONS_TOOL,
            LIST_LIVE_DASHBOARDS_TOOL,
            GET_LIVE_DASHBOARD_TOOL,
            TEST_RUN_LIVE_DASHBOARD_TOOL,
            LIST_GOLDEN_QUERIES_TOOL,
            FETCH_GOLDEN_QUERY_TOOL,
            REPORT_FAILED_SEARCH_TOOL,
            CLICKHOUSE_QUERY_TOOL,
        ],
    };

    json!(tools)
}

pub fn get_database_schema_value(table_name: Option<&str>) -> serde_json::Value {
    let schema = json!({
        "tables": [
            {
                "name": "user_events_v1",
                "description": "Optimised table for user behavior analytics and run time session creations, cohorts creation",
                "columns": [
                    { "name": "event_id", "type": "UUID" },
                    { "name": "user_id", "type": "String" },
                    { "name": "anon_id", "type": "String" },
                    { "name": "event_name", "type": "LowCardinality(String)" },
                    { "name": "event_timestamp", "type": "DateTime64(3)" },
                    { "name": "event_date", "type": "Date" },
                    { "name": "event_hour", "type": "DateTime" },
                    { "name": "properties", "type": "JSON" },
                    { "name": "device_type", "type": "LowCardinality(String)" },
                    { "name": "platform", "type": "LowCardinality(String)" },
                    { "name": "browser", "type": "LowCardinality(String)" },
                    { "name": "country", "type": "LowCardinality(Nullable(FixedString(2)))" },
                    { "name": "city", "type": "String" },
                    { "name": "user_agent", "type": "String" },
                    { "name": "inserted_at", "type": "DateTime" },
                    { "name": "version", "type": "String" }
                ],
                "orderBy": ["user_id", "event_timestamp", "event_name", "event_id"],
                "partitionBy": "toYYYYMM(event_date)"
            },
            {
                "name": "events_v1",
                "description": "Optimised for event analytics. Use this table for event-related queries (event names, event properties, event counts) as event_name is the primary sort key.",
                "columns": [
                    { "name": "event_id", "type": "UUID" },
                    { "name": "event_name", "type": "LowCardinality(String)" },
                    { "name": "event_timestamp", "type": "DateTime64(3)" },
                    { "name": "event_date", "type": "Date" },
                    { "name": "event_hour", "type": "DateTime" },
                    { "name": "properties", "type": "JSON" },
                    { "name": "device_type", "type": "LowCardinality(String)" },
                    { "name": "platform", "type": "LowCardinality(String)" },
                    { "name": "browser", "type": "LowCardinality(String)" },
                    { "name": "country", "type": "LowCardinality(Nullable(FixedString(2)))" },
                    { "name": "city", "type": "String" },
                    { "name": "user_agent", "type": "String" },
                    { "name": "inserted_at", "type": "DateTime" },
                    { "name": "version", "type": "String" }
                ],
                "orderBy": ["event_name", "event_timestamp", "event_id"],
                "partitionBy": "toYYYYMM(event_date)"
            },
            {
                "name": "event_schema_catalog",
                "description": "Optimised to get glossary of events",
                "columns": [
                    { "name": "proj_id", "type": "UUID" },
                    { "name": "event_name", "type": "String" },
                    { "name": "property", "type": "String" },
                    { "name": "type", "type": "String" },
                    { "name": "description", "type": "String" }
                ],
                "orderBy": ["proj_id", "event_name", "property", "type"]
            }
        ]
    });

    if let Some(name) = table_name {
        if let Some(tables) = schema["tables"].as_array() {
            if let Some(table) = tables.iter().find(|t| t["name"] == name) {
                return json!({ "tables": [table] });
            }
        }
        return json!({ "tables": [], "error": format!("Table '{}' not found", name) });
    }

    schema
}
