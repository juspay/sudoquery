//! clickhouse-proxy: queries, workspace provisioning and the tables behind
//! SQL consoles and event descriptions. See `server` for the API.

use clickhouse_proxy::clickhouse::{ClickHouse, ClickHouseConfig};
use clickhouse_proxy::server::{ProxyState, router};
use clickhouse_proxy::store;

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let clickhouse = ClickHouse::new(ClickHouseConfig {
        url: env_or("CLICKHOUSE_URL", "http://localhost:8123"),
        admin_url: env_or("CLICKHOUSE_ADMIN_URL", "http://localhost:8123"),
        admin_user: env_or("CLICKHOUSE_ADMIN_USER", "default"),
        admin_password: std::env::var("CLICKHOUSE_ADMIN_PASSWORD").unwrap_or_default(),
        workspace_password: std::env::var("CLICKHOUSE_WORKSPACE_PASSWORD").unwrap_or_default(),
    });

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let schema = env_or("DATABASE_SCHEMA", "clickhouse_proxy");
    let pool = store::connect(&database_url, &schema)
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let token = std::env::var("CLICKHOUSE_PROXY_TOKEN")
        .ok()
        .map(|token| token.trim().to_owned())
        .filter(|token| !token.is_empty());
    if token.is_none() {
        tracing::warn!(
            "CLICKHOUSE_PROXY_TOKEN is not set; any caller can query, provision and drop any workspace"
        );
    }

    let app = router(ProxyState {
        clickhouse,
        pool,
        kafka_brokers: env_or("CLICKHOUSE_KAFKA_BROKERS", "redpanda:9092"),
        token,
    });
    let addr = env_or("CLICKHOUSE_PROXY_ADDR", "0.0.0.0:3003");
    tracing::info!("Listening on {}", addr);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
