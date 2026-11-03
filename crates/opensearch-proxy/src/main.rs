//! opensearch-proxy: event search over the events sink-opensearch indexed,
//! for dashboard-server and the CDP dashboard. See `server` for the API.

use std::time::Duration;

use opensearch_proxy::OpenSearch;
use opensearch_proxy::server::{ProxyState, router};

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| default.to_owned())
}

fn credentials() -> Option<(String, String)> {
    let username = std::env::var("OPENSEARCH_USERNAME").ok();
    let password = std::env::var("OPENSEARCH_PASSWORD").ok();
    match (username, password) {
        (Some(username), Some(password)) => Some((username, password)),
        (None, None) => None,
        _ => panic!("OPENSEARCH_USERNAME and OPENSEARCH_PASSWORD must be set together"),
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let timeout_ms: u64 = env_or("OPENSEARCH_REQUEST_TIMEOUT_MS", "30000")
        .parse()
        .expect("OPENSEARCH_REQUEST_TIMEOUT_MS must be a number of milliseconds");
    let opensearch = OpenSearch::new(
        &env_or("OPENSEARCH_URL", "http://localhost:9200"),
        credentials(),
        // Must name the data stream sink-opensearch writes to; `{org_id}` and
        // `{workspace_id}` are filled in per request.
        env_or("OPENSEARCH_INDEX", "events-{org_id}-{workspace_id}"),
        Duration::from_millis(timeout_ms),
    )
    .expect("Failed to create OpenSearch client");

    let token = std::env::var("OPENSEARCH_PROXY_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty());
    if token.is_none() {
        tracing::warn!("OPENSEARCH_PROXY_TOKEN is not set; any caller can search any workspace");
    }

    let app = router(ProxyState { opensearch, token });
    let addr = env_or("OPENSEARCH_PROXY_ADDR", "0.0.0.0:3002");
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
