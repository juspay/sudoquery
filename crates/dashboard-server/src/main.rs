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

mod clickhouse_relay;
mod db;
mod entities;
mod keycloak;
mod middleware;
mod models;
mod provisioning;
mod routes;
mod search_proxy;
mod state;

use clickhouse_proxy::client::ClickHouseProxy;
use keycloak::KeycloakAdmin;
use middleware::auth::{AuthUser, WorkspaceAccess};
use models::ChatCompletionRequest;
use opensearch_proxy::OpenSearch;
use sea_orm::Database;
use search_proxy::SearchProxy;
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

    // ClickHouse, as clickhouse-proxy reaches it
    let ch_status = if s.clickhouse.clickhouse_connected().await {
        "connected"
    } else {
        tracing::error!("Health check: ClickHouse (through clickhouse-proxy) is unreachable");
        "disconnected"
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
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
) -> impl IntoResponse {
    let timezone = workspace.timezone.as_deref().unwrap_or("Asia/Kolkata");
    match s
        .clickhouse
        .today_count(workspace.id.as_ref(), timezone)
        .await
    {
        Ok(count) => Json(json!({ "count": count })),
        Err(e) => {
            tracing::error!("Failed to fetch today's event count: {}", e);
            Json(json!({ "count": 0, "error": "Failed to fetch event count" }))
        }
    }
}

async fn get_events(
    State(s): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
) -> impl IntoResponse {
    match s.clickhouse.event_names(workspace.id.as_ref()).await {
        Ok(event_names) => Json(json!(event_names)),
        Err(e) => {
            tracing::error!("Failed to fetch events: {}", e);
            Json(json!({"error": "Failed to fetch events"}))
        }
    }
}

async fn get_event_props(
    State(s): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let event_name = match params.get("event_name") {
        Some(name) => name,
        None => return Json(json!({"error": "Missing event_name parameter"})),
    };

    match s
        .clickhouse
        .event_properties(workspace.id.as_ref(), event_name)
        .await
    {
        Ok(keys) => Json(json!(keys)),
        Err(e) => {
            tracing::error!("Failed to fetch event properties: {}", e);
            Json(json!({"error": "Failed to fetch event properties"}))
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawQueryRequest {
    query: String,
}

async fn run_raw_query(
    State(s): State<AppState>,
    WorkspaceAccess { workspace, .. }: WorkspaceAccess,
    Json(req): Json<RawQueryRequest>,
) -> impl IntoResponse {
    let trimmed_query = req.query.trim();

    if trimmed_query.is_empty() {
        return Json(json!({"error": "Empty query"}));
    }

    match s
        .clickhouse
        .query(workspace.id.as_ref(), trimmed_query)
        .await
    {
        Ok(response) => Json(json!({"success": true, "response": response})),
        Err(e) => Json(json!({"success": false, "error": e.public_message()})),
    }
}

// clickhouse-proxy, which runs every ClickHouse query and provisions each
// workspace's ClickHouse database and user.
fn get_clickhouse_proxy_url() -> String {
    std::env::var("CLICKHOUSE_PROXY_URL").unwrap_or_else(|_| "http://localhost:3003".to_string())
}

fn get_clickhouse_proxy_token() -> Option<String> {
    std::env::var("CLICKHOUSE_PROXY_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

// Long enough for a heavy analytics query and for provisioning a database.
fn get_clickhouse_proxy_timeout() -> std::time::Duration {
    let millis = match std::env::var("CLICKHOUSE_PROXY_TIMEOUT_MS") {
        Ok(value) => value
            .parse()
            .expect("CLICKHOUSE_PROXY_TIMEOUT_MS must be a number of milliseconds"),
        Err(_) => 120_000,
    };
    std::time::Duration::from_millis(millis)
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

// Must name the data stream sink-opensearch writes to; `{org_id}` and `{workspace_id}` are
// filled in per request.
fn get_opensearch_index() -> String {
    std::env::var("OPENSEARCH_INDEX")
        .unwrap_or_else(|_| "events-{org_id}-{workspace_id}".to_string())
}

fn get_admin_api_token() -> Option<String> {
    std::env::var("ADMIN_API_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

fn get_opensearch_credentials() -> Option<(String, String)> {
    credentials_from_env("OPENSEARCH_USERNAME", "OPENSEARCH_PASSWORD")
}

// Creates and deletes data streams during provisioning; unset, the read credentials do.
fn get_opensearch_admin_credentials() -> Option<(String, String)> {
    credentials_from_env("OPENSEARCH_ADMIN_USERNAME", "OPENSEARCH_ADMIN_PASSWORD")
}

fn credentials_from_env(username_var: &str, password_var: &str) -> Option<(String, String)> {
    let username = std::env::var(username_var).ok();
    let password = std::env::var(password_var).ok();
    match (username, password) {
        (Some(username), Some(password)) => Some((username, password)),
        (None, None) => None,
        _ => panic!("{username_var} and {password_var} must be set together"),
    }
}

// opensearch-proxy, which runs event searches for the Discover routes.
fn get_opensearch_proxy_url() -> String {
    std::env::var("OPENSEARCH_PROXY_URL").unwrap_or_else(|_| "http://localhost:3002".to_string())
}

fn get_opensearch_proxy_token() -> Option<String> {
    std::env::var("OPENSEARCH_PROXY_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty())
}

// A search is a validation then the search itself, each up to the proxy's
// own OpenSearch timeout, so this allows for both.
fn get_opensearch_proxy_timeout() -> std::time::Duration {
    let millis = match std::env::var("OPENSEARCH_PROXY_TIMEOUT_MS") {
        Ok(value) => value
            .parse()
            .expect("OPENSEARCH_PROXY_TIMEOUT_MS must be a number of milliseconds"),
        Err(_) => 65_000,
    };
    std::time::Duration::from_millis(millis)
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

async fn chat_completions(
    _auth_user: AuthUser,
    Json(req): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
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
                    Err(_) => {
                        tracing::error!(
                            "Invalid JSON response from LLM API ({}): {}",
                            status,
                            body
                        );
                        Json(json!({"error": "LLM request failed"}))
                    }
                },
                Err(e) => {
                    tracing::error!("Failed to read LLM API response: {}", e);
                    Json(json!({"error": "LLM request failed"}))
                }
            }
        }
        Err(e) => {
            tracing::error!("LLM API request failed: {}", e);
            Json(json!({"error": "LLM request failed"}))
        }
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

    let clickhouse = ClickHouseProxy::new(
        &get_clickhouse_proxy_url(),
        get_clickhouse_proxy_token(),
        get_clickhouse_proxy_timeout(),
    )
    .expect("Failed to create the clickhouse-proxy client");
    let provisioning = provisioning::ProvisioningConfig::from_env(get_kafka_broker_url())
        .unwrap_or_else(|e| panic!("invalid workspace provisioning configuration: {e}"));

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
    .expect("Failed to create OpenSearch client")
    .with_admin_credentials(get_opensearch_admin_credentials());

    let search_proxy = SearchProxy::new(
        &get_opensearch_proxy_url(),
        get_opensearch_proxy_token(),
        get_opensearch_proxy_timeout(),
    )
    .expect("Failed to create the opensearch-proxy client");

    let admin_api_token = get_admin_api_token();
    if admin_api_token.is_none() {
        tracing::warn!("ADMIN_API_TOKEN is not set; the admin API is disabled");
    }

    let state = AppState {
        http: Client::new(),
        litellm_url: get_llm_api_endpoint(),
        litellm_key: get_llm_api_key(),
        db_pool: db_pool.clone(),
        db_conn,
        keycloak,
        keycloak_url,
        keycloak_realm,
        clickhouse,
        opensearch,
        search_proxy,
        provisioning,
        admin_api_token,
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
        .route(
            "/my/workspaces",
            get(routes::workspaces::list_my_workspaces),
        )
        // Workspace routes
        .route("/workspaces", post(routes::workspaces::create_workspace))
        .route("/workspaces", get(routes::workspaces::list_workspaces))
        .route("/workspace", get(routes::workspaces::get_workspace))
        .route("/workspace", patch(routes::workspaces::update_workspace))
        .route("/workspace", delete(routes::workspaces::delete_workspace))
        .route(
            "/workspace/tokens",
            get(routes::workspaces::get_workspace_tokens),
        )
        .route("/timezones", get(routes::workspaces::get_timezones))
        // Admin routes (X-Admin-Token)
        .route(
            "/admin/workspaces/{workspace_id}",
            get(routes::admin::get_workspace_provisioning),
        )
        .route(
            "/admin/workspaces/{workspace_id}/provision",
            post(routes::admin::provision_workspace),
        )
        // Workspace member routes
        .route(
            "/workspace/members",
            post(routes::users::add_workspace_member),
        )
        .route(
            "/workspace/members/{user_id}",
            delete(routes::users::remove_workspace_member),
        )
        .route(
            "/workspace/members",
            get(routes::users::list_workspace_members),
        )
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
            "/workspace/invitations",
            post(routes::invitations::create_workspace_invitation),
        )
        .route(
            "/workspace/invitations",
            get(routes::invitations::list_workspace_invitations),
        )
        // Event description routes
        .route(
            "/workspace/event-description",
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
            "/workspace/property-description",
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
        .route("/workspace/chats", get(routes::chats::list_chats))
        .route(
            "/workspace/chat/{chat_id}",
            delete(routes::chats::delete_chat),
        )
        .route(
            "/workspace/chat/llm_chatv1",
            post(routes::llm_chatv1::utils::chat_handler_v1),
        )
        .route(
            "/workspace/chat/gen_titlev1",
            post(routes::llm_chatv1::title_gen::generate_title_handler),
        )
        .route(
            "/workspace/chat/{chat_id}/messages",
            get(routes::llm_chatv1::utils::get_chat_messages),
        )
        .route(
            "/workspace/chat/client_tool_response_v1",
            post(routes::llm_chatv1::utils::client_tool_call_response_v1),
        )
        .route(
            "/workspace/chat/get_metric_data_v1",
            post(routes::llm_chatv1::metric_data::get_metric_data_handler),
        )
        .route(
            "/workspace/chat/{chat_id}/save-dashboard",
            post(routes::save_live_dashboard::save_live_dashboard_from_tool_call),
        )
        // Live dashboard routes
        .route(
            "/workspace/live-dashboards",
            post(routes::live_dashboards::create_dashboard),
        )
        .route(
            "/workspace/live-dashboards",
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
            "/workspace/consoles",
            post(routes::user_workspace_consoles::create_console),
        )
        .route(
            "/workspace/consoles",
            get(routes::user_workspace_consoles::list_consoles),
        )
        .route(
            "/workspace/console",
            get(routes::user_workspace_consoles::get_console),
        )
        .route(
            "/workspace/console",
            patch(routes::user_workspace_consoles::update_console),
        )
        .route(
            "/workspace/console",
            delete(routes::user_workspace_consoles::delete_console),
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
