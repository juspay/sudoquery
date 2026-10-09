use axum::{
    Json, Router,
    extract::State,
    response::IntoResponse,
    routing::{delete, get, patch, post},
};
use rdkafka::config::ClientConfig;
use rdkafka::producer::{FutureProducer, Producer};
use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod clickhouse;
mod db;
mod entities;
mod ids;
mod keycloak;
mod middleware;
mod models;
mod opensearch;
mod routes;
mod state;

use keycloak::KeycloakAdmin;
use middleware::auth::ProjectAccess;
use models::ChatCompletionRequest;
use opensearch::OpenSearch;
use sea_orm::Database;
use sqlx::postgres::PgPoolOptions;
use state::AppState;

async fn health_check(State(s): State<AppState>) -> impl IntoResponse {
    // Test PostgreSQL connection
    let pg_status = match sqlx::query("SELECT 1").fetch_one(&s.db_pool).await {
        Ok(_) => "connected",
        Err(e) => {
            tracing::error!("Health check: PostgreSQL connection failed: {}", e);
            "disconnected"
        }
    };

    // Test ClickHouse connection (using admin URL)
    let ch_status = match clickhouse::execute_admin_query(
        &Client::new(),
        &s.clickhouse_admin_url,
        &s.clickhouse_admin_user,
        &s.clickhouse_admin_password,
        "SELECT 1",
    )
    .await
    {
        Ok(_) => "connected",
        Err(e) => {
            tracing::error!("Health check: ClickHouse connection failed: {}", e);
            "disconnected"
        }
    };

    // Test Redpanda (Kafka) connection
    let kafka_probe: Result<FutureProducer, rdkafka::error::KafkaError> = ClientConfig::new()
        .set("bootstrap.servers", get_kafka_broker_url())
        .create();
    let kafka_status = match kafka_probe.and_then(|producer| {
        producer
            .client()
            .fetch_metadata(None, std::time::Duration::from_secs(3))
            .map(|_| ())
    }) {
        Ok(_) => "connected",
        Err(e) => {
            tracing::error!("Health check: Redpanda/Kafka connection failed: {}", e);
            "disconnected"
        }
    };

    Json(json!({
        "running": true,
        "postgres": pg_status,
        "clickhouse": ch_status,
        "redpanda": kafka_status,
    }))
}

async fn get_events_today_count(
    State(s): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> impl IntoResponse {
    let timezone = project.timezone.as_deref().unwrap_or("Asia/Kolkata");
    let query = format!(
        "SELECT count() as count FROM events_v2 WHERE toDate(event_timestamp, '{}') = toDate(now('{}'))",
        timezone, timezone
    );
    match clickhouse::execute_project_query(
        &s.clickhouse_url,
        &project.id,
        &s.clickhouse_project_password,
        &query,
    )
    .await
    {
        Ok(result) => {
            let count = result["data"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|row| row["count"].as_u64())
                .unwrap_or(0);
            Json(json!({ "count": count }))
        }
        Err(e) => Json(json!({ "count": 0, "error": e.to_string() })),
    }
}

async fn get_events(
    State(s): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
) -> impl IntoResponse {
    let query = "SELECT event_name FROM events_v2 GROUP BY event_name";
    match clickhouse::execute_project_query(
        &s.clickhouse_url,
        &project.id,
        &s.clickhouse_project_password,
        query,
    )
    .await
    {
        Ok(result) => {
            let event_names: Vec<String> = result["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|row| row["event_name"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Json(json!(event_names))
        }
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

async fn get_event_props(
    State(s): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let event_name = match params.get("event_name") {
        Some(name) => name,
        None => return Json(json!({"error": "Missing event_name parameter"})),
    };

    let query = format!(
        "SELECT DISTINCT key \
        FROM \
        ( \
            SELECT * \
            FROM events_v2 \
            WHERE event_name = '{}' \
        ) \
        ARRAY JOIN JSONAllPaths(properties) AS key",
        event_name
    );

    match clickhouse::execute_project_query(
        &s.clickhouse_url,
        &project.id,
        &s.clickhouse_project_password,
        &query,
    )
    .await
    {
        Ok(result) => {
            let mut keys: Vec<String> = result["data"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|row| row["key"].as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            keys.sort();
            Json(json!(keys))
        }
        Err(e) => Json(json!({"error": e.to_string()})),
    }
}

#[derive(Debug, Deserialize)]
struct RawQueryRequest {
    query: String,
}

async fn run_raw_query(
    State(s): State<AppState>,
    ProjectAccess { project, .. }: ProjectAccess,
    Json(req): Json<RawQueryRequest>,
) -> impl IntoResponse {
    let trimmed_query = req.query.trim();

    if trimmed_query.is_empty() {
        return Json(json!({"error": "Empty query"}));
    }

    match clickhouse::execute_project_query(
        &s.clickhouse_url,
        &project.id,
        &s.clickhouse_project_password,
        trimmed_query,
    )
    .await
    {
        Ok(response) => Json(json!({"success": true, "response": response})),
        Err(e) => Json(json!({"success": false, "error": e.to_string()})),
    }
}

fn get_clickhouse_url() -> String {
    std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string())
}

fn get_clickhouse_admin_url() -> String {
    std::env::var("CLICKHOUSE_ADMIN_URL").unwrap_or_else(|_| "http://localhost:8123".to_string())
}

fn get_clickhouse_admin_user() -> String {
    std::env::var("CLICKHOUSE_ADMIN_USER").unwrap_or_else(|_| "default".to_string())
}

fn get_clickhouse_admin_password() -> String {
    std::env::var("CLICKHOUSE_ADMIN_PASSWORD").unwrap_or_default()
}

fn get_clickhouse_project_password() -> String {
    std::env::var("CLICKHOUSE_PROJECT_PASSWORD").unwrap_or_default()
}

fn get_bind_addr() -> String {
    std::env::var("DASHBOARD_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:3000".to_string())
}

fn get_llm_api_key() -> String {
    std::env::var("LLM_API_KEY").unwrap_or_default()
}

fn get_llm_api_endpoint() -> String {
    std::env::var("LLM_API_ENDPOINT")
        .unwrap_or_else(|_| "https://grid.ai.juspay.net/v1/chat/completions".to_string())
}

fn get_kafka_broker_url() -> String {
    std::env::var("KAFKA_BROKER_URL").unwrap_or_else(|_| "localhost:19092".to_string())
}

fn get_opensearch_url() -> String {
    std::env::var("OPENSEARCH_URL").unwrap_or_else(|_| "http://localhost:9200".to_string())
}

// Must name the index sink-opensearch writes to; `{org_id}` is filled in per request.
fn get_opensearch_index() -> String {
    std::env::var("OPENSEARCH_INDEX").unwrap_or_else(|_| "events-{org_id}".to_string())
}

fn get_opensearch_credentials() -> Option<(String, String)> {
    let username = std::env::var("OPENSEARCH_USERNAME").ok();
    let password = std::env::var("OPENSEARCH_PASSWORD").ok();
    match (username, password) {
        (Some(username), Some(password)) => Some((username, password)),
        (None, None) => None,
        _ => panic!("OPENSEARCH_USERNAME and OPENSEARCH_PASSWORD must be set together"),
    }
}

fn get_opensearch_request_timeout() -> std::time::Duration {
    let millis = match std::env::var("OPENSEARCH_REQUEST_TIMEOUT_MS") {
        Ok(value) => value
            .parse()
            .expect("OPENSEARCH_REQUEST_TIMEOUT_MS must be a number of milliseconds"),
        Err(_) => 30_000,
    };
    std::time::Duration::from_millis(millis)
}

async fn chat_completions(Json(req): Json<ChatCompletionRequest>) -> impl IntoResponse {
    let api_key = get_llm_api_key();
    if api_key.is_empty() {
        return Json(json!({"error": "LLM_API_KEY not configured on server"}));
    }

    let endpoint = get_llm_api_endpoint();
    let client = reqwest::Client::new();

    let response = client
        .post(&endpoint)
        .header("Content-Type", "application/json")
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&req)
        .send()
        .await;

    match response {
        Ok(resp) => {
            let status = resp.status();
            match resp.text().await {
                Ok(body) => match serde_json::from_str::<serde_json::Value>(&body) {
                    Ok(json_response) => Json(json_response),
                    Err(_) => Json(json!({
                        "error": format!("Invalid JSON response from LLM API: {}", body),
                        "status": status.as_u16()
                    })),
                },
                Err(e) => Json(json!({"error": format!("Failed to read LLM API response: {}", e)})),
            }
        }
        Err(e) => Json(json!({"error": format!("LLM API request failed: {}", e)})),
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    dotenvy::dotenv().ok();

    let clickhouse_url = get_clickhouse_url();
    let clickhouse_admin_url = get_clickhouse_admin_url();
    let clickhouse_admin_user = get_clickhouse_admin_user();
    let clickhouse_admin_password = get_clickhouse_admin_password();
    let clickhouse_project_password = get_clickhouse_project_password();

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let db_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    let keycloak_url = std::env::var("KEYCLOAK_URL").expect("KEYCLOAK_URL must be set");
    println!("keycloak_url {}", &keycloak_url);
    let keycloak_admin_user =
        std::env::var("KEYCLOAK_ADMIN_USER").expect("KEYCLOAK_ADMIN_USER must be set");
    let keycloak_admin_pass =
        std::env::var("KEYCLOAK_ADMIN_PASS").expect("KEYCLOAK_ADMIN_PASS must be set");
    let keycloak_realm =
        std::env::var("KEYCLOAK_REALM").unwrap_or_else(|_| "hyper-analytics".to_string());
    let temp_password =
        std::env::var("TEMP_PASSWORD").unwrap_or_else(|_| "ChangeMe123!".to_string());

    let keycloak = KeycloakAdmin::new(
        &keycloak_url,
        &keycloak_admin_user,
        &keycloak_admin_pass,
        &keycloak_realm,
    );

    // Initialize SeaORM connection
    let db_conn = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database with SeaORM");

    let opensearch = OpenSearch::new(
        &get_opensearch_url(),
        get_opensearch_credentials(),
        get_opensearch_index(),
        get_opensearch_request_timeout(),
    )
    .expect("Failed to create OpenSearch client");

    let state = AppState {
        http: Client::new(),
        litellm_url: get_llm_api_endpoint(),
        litellm_key: get_llm_api_key(),
        db_pool: db_pool.clone(),
        db_conn,
        keycloak,
        keycloak_url,
        keycloak_realm,
        temp_password,
        clickhouse_url,
        clickhouse_admin_url,
        clickhouse_admin_user,
        clickhouse_admin_password,
        clickhouse_project_password,
        opensearch,
    };

    let api_routes = Router::new()
        .route("/health", get(health_check))
        .route("/events", get(get_events))
        .route("/events/today/count", get(get_events_today_count))
        .route("/event_props", get(get_event_props))
        .route("/query", post(run_raw_query))
        .route("/chat/completions", post(chat_completions))
        .route(
            "/chat/llm_chat",
            post(routes::llm_chat::utils::chat_handler),
        )
        .route(
            "/chat/llm_chatv1",
            post(routes::llm_chatv1::utils::chat_handler_v1),
        )
        // Organization routes
        .route(
            "/organizations",
            post(routes::organizations::create_organization),
        )
        .route(
            "/organization",
            delete(routes::organizations::delete_organization),
        )
        .route(
            "/organization",
            get(routes::organizations::get_organization),
        )
        .route(
            "/organization",
            patch(routes::organizations::update_organization),
        )
        .route(
            "/organization/members",
            post(routes::organizations::add_organization_member),
        )
        .route(
            "/organization/members/{user_id}",
            delete(routes::organizations::remove_organization_member),
        )
        .route(
            "/organization/members/{user_id}/role",
            post(routes::organizations::update_organization_member_role),
        )
        .route(
            "/organization/members",
            get(routes::organizations::list_organization_members),
        )
        // User routes
        .route("/users", post(routes::users::create_user))
        .route("/users/{user_id}", delete(routes::users::delete_user))
        .route("/me", get(routes::users::me))
        // My routes
        .route(
            "/my/organizations",
            get(routes::organizations::list_my_organizations),
        )
        .route("/my/projects", get(routes::projects::list_my_projects))
        // Project routes
        .route("/projects", post(routes::projects::create_project))
        .route("/projects", get(routes::projects::list_projects))
        .route("/project", get(routes::projects::get_project))
        .route("/project", patch(routes::projects::update_project))
        .route("/project", delete(routes::projects::delete_project))
        .route("/project/tokens", get(routes::projects::get_project_tokens))
        .route("/timezones", get(routes::projects::get_timezones))
        // Project member routes
        .route("/project/members", post(routes::users::add_project_member))
        .route(
            "/project/members/{user_id}",
            delete(routes::users::remove_project_member),
        )
        .route("/project/members", get(routes::users::list_project_members))
        // Invitation routes
        .route(
            "/invitations/my",
            get(routes::invitations::list_my_invitations),
        )
        .route(
            "/invitations/sent",
            get(routes::invitations::list_sent_invitations),
        )
        .route(
            "/invitations/{invitation_id}/accept",
            post(routes::invitations::accept_invitation),
        )
        .route(
            "/invitations/{invitation_id}/revoke",
            post(routes::invitations::revoke_invitation),
        )
        .route(
            "/organization/invitations",
            post(routes::invitations::create_organization_invitation),
        )
        .route(
            "/organization/invitations",
            get(routes::invitations::list_organization_invitations),
        )
        .route(
            "/project/invitations",
            post(routes::invitations::create_project_invitation),
        )
        .route(
            "/project/invitations",
            get(routes::invitations::list_project_invitations),
        )
        // Event description routes
        .route(
            "/project/event-description",
            post(routes::event_descriptions::upsert_event_description),
        )
        .route(
            "/event-descriptions",
            get(routes::event_descriptions::list_event_descriptions),
        )
        .route(
            "/event-description",
            get(routes::event_descriptions::get_event_description),
        )
        // Property description routes
        .route(
            "/project/property-description",
            post(routes::property_descriptions::upsert_property_description),
        )
        .route(
            "/property-descriptions",
            get(routes::property_descriptions::list_property_descriptions),
        )
        .route(
            "/property-description",
            get(routes::property_descriptions::get_property_description),
        )
        // Chat routes
        .route("/project/chats", get(routes::chats::list_chats))
        .route(
            "/project/chat/{chat_id}",
            delete(routes::chats::delete_chat),
        )
        .route(
            "/project/chat/llm_chatv1",
            post(routes::llm_chatv1::utils::chat_handler_v1),
        )
        .route(
            "/project/chat/gen_titlev1",
            post(routes::llm_chatv1::title_gen::generate_title_handler),
        )
        .route(
            "/project/chat/{chat_id}/messages",
            get(routes::llm_chatv1::utils::get_chat_messages),
        )
        .route(
            "/project/chat/client_tool_response_v1",
            post(routes::llm_chatv1::utils::client_tool_call_response_v1),
        )
        .route(
            "/project/chat/get_metric_data_v1",
            post(routes::llm_chatv1::metric_data::get_metric_data_handler),
        )
        .route(
            "/project/chat/{chat_id}/save-dashboard",
            post(routes::save_live_dashboard::save_live_dashboard_from_tool_call),
        )
        // Live dashboard routes
        .route(
            "/project/live-dashboards",
            post(routes::live_dashboards::create_dashboard),
        )
        .route(
            "/project/live-dashboards",
            get(routes::live_dashboards::list_dashboards),
        )
        .route(
            "/live-dashboard",
            get(routes::live_dashboards::get_dashboard),
        )
        .route(
            "/live-dashboard",
            patch(routes::live_dashboards::update_dashboard),
        )
        .route(
            "/live-dashboard",
            delete(routes::live_dashboards::delete_dashboard),
        )
        .route(
            "/live-dashboard/test-run",
            post(routes::live_dashboards::test_run_dashboard),
        )
        .route(
            "/project/consoles",
            post(routes::user_project_consoles::create_console),
        )
        .route(
            "/project/consoles",
            get(routes::user_project_consoles::list_consoles),
        )
        .route(
            "/project/console",
            get(routes::user_project_consoles::get_console),
        )
        .route(
            "/project/console",
            patch(routes::user_project_consoles::update_console),
        )
        .route(
            "/project/console",
            delete(routes::user_project_consoles::delete_console),
        )
        // Event search routes (OpenSearch)
        .route("/search", post(routes::search::search))
        .route("/count", post(routes::search::count))
        .route("/doc/{doc_id}", get(routes::search::get_doc))
        .route("/session", post(routes::sessions::list_sessions))
        .route("/session/{session_id}", post(routes::sessions::get_session))
        .route("/histogram", post(routes::aggregations::histogram))
        .route("/facets", post(routes::aggregations::facets))
        .with_state(state);

    let router = Router::new().nest("/api", api_routes);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let app = router.layer(cors);

    let addr = get_bind_addr();
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
