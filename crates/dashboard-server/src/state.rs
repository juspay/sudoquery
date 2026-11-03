use reqwest::Client;
use sea_orm::DatabaseConnection;
use sqlx::PgPool;

use crate::keycloak::KeycloakAdmin;
use crate::provisioning::ProvisioningConfig;
use crate::search_proxy::SearchProxy;
use clickhouse_proxy::client::ClickHouseProxy;
use opensearch_proxy::OpenSearch;

#[derive(Clone)]
pub struct AppState {
    pub http: Client,
    pub litellm_url: String,
    pub litellm_key: String,
    pub db_pool: PgPool,
    pub db_conn: DatabaseConnection,
    pub keycloak: KeycloakAdmin,
    pub keycloak_url: String,
    pub keycloak_realm: String,
    /// Every ClickHouse query and workspace database, through clickhouse-proxy.
    pub clickhouse: ClickHouseProxy,
    /// Provisioning only: creates and deletes workspace data streams.
    pub opensearch: OpenSearch,
    /// Event search, run by opensearch-proxy.
    pub search_proxy: SearchProxy,
    pub provisioning: ProvisioningConfig,
    /// Token the admin API requires (`ADMIN_API_TOKEN`); `None` disables it.
    pub admin_api_token: Option<String>,
}
