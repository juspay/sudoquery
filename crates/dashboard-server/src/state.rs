use reqwest::Client;
use sea_orm::DatabaseConnection;
use sqlx::PgPool;

use crate::keycloak::KeycloakAdmin;
use crate::opensearch::OpenSearch;

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
    pub temp_password: String,
    pub clickhouse_url: String,
    pub clickhouse_admin_url: String,
    pub clickhouse_admin_user: String,
    pub clickhouse_admin_password: String,
    pub clickhouse_project_password: String,
    pub opensearch: OpenSearch,
}
