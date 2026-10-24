use canonical_event::WorkspaceId;
use reqwest::Client;
use serde_json::Value;

#[derive(Debug, thiserror::Error)]
pub enum ClickHouseError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Query error: {0}")]
    Query(String),
}

pub async fn create_workspace_user(
    admin_url: &str,
    admin_user: &str,
    admin_password: &str,
    workspace_password: &str,
    workspace_id: &WorkspaceId,
) -> Result<(), ClickHouseError> {
    let client = Client::new();
    let workspace_id_str = workspace_id.to_string();

    let create_query = format!(
        "CREATE USER IF NOT EXISTS '{}' IDENTIFIED WITH sha256_password BY '{}'",
        workspace_id_str, workspace_password
    );
    execute_admin_query(
        &client,
        admin_url,
        admin_user,
        admin_password,
        &create_query,
    )
    .await?;

    // Read access to the workspace's own database (provisioning.rs), and
    // nothing else. Only the tables: never its Kafka table, which a SELECT
    // would consume from.
    for table in crate::provisioning::WORKSPACE_TABLES {
        let grant_query = format!(
            "GRANT SELECT ON `{}`.{} TO '{}'",
            workspace_id_str, table, workspace_id_str
        );
        execute_admin_query(&client, admin_url, admin_user, admin_password, &grant_query).await?;
    }

    Ok(())
}

pub async fn drop_workspace_user(
    admin_url: &str,
    admin_user: &str,
    admin_password: &str,
    workspace_id: &WorkspaceId,
) -> Result<(), ClickHouseError> {
    let client = Client::new();
    let workspace_id_str = workspace_id.to_string();

    let drop_query = format!("DROP USER IF EXISTS '{}'", workspace_id_str);
    execute_admin_query(&client, admin_url, admin_user, admin_password, &drop_query).await?;

    Ok(())
}

pub async fn execute_admin_query(
    client: &Client,
    url: &str,
    user: &str,
    password: &str,
    query: &str,
) -> Result<String, ClickHouseError> {
    let response = client
        .post(url)
        .basic_auth(user, Some(password))
        .body(query.to_string())
        .send()
        .await
        .map_err(|e| ClickHouseError::Http(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(ClickHouseError::Query(body));
    }

    Ok(body)
}

pub async fn execute_query(clickhouse_url: &str, query: &str) -> Result<Value, ClickHouseError> {
    let client = Client::new();
    let formatted_query = format!("{} FORMAT JSON", query.trim());

    let response = client
        .post(clickhouse_url)
        .body(formatted_query)
        .send()
        .await
        .map_err(|e| ClickHouseError::Http(e.to_string()))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        return Err(ClickHouseError::Query(body));
    }

    serde_json::from_str(&body).map_err(|e| ClickHouseError::Query(e.to_string()))
}

pub async fn execute_workspace_query(
    clickhouse_url: &str,
    workspace_id: &WorkspaceId,
    workspace_password: &str,
    query: &str,
) -> Result<Value, ClickHouseError> {
    let client = Client::new();
    let workspace_id_str = workspace_id.to_string();
    let formatted_query = format!("{} FORMAT JSON", query.trim());

    println!(
        "Executing query for workspace {}: {}",
        workspace_id_str, query
    );

    // Every workspace has its own database, named after the workspace
    // (provisioning.rs), so unqualified table names resolve to its tables.
    let url = format!("{}?database={}", clickhouse_url, workspace_id_str);

    let response = client
        .post(&url)
        .basic_auth(&workspace_id_str, Some(workspace_password))
        .body(formatted_query)
        .send()
        .await
        .map_err(|e| ClickHouseError::Http(e.to_string()))?;

    let status = response.status();
    println!("Received response with status: {}", status);
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        tracing::error!("ClickHouse error for workspace {}: {}", workspace_id, body);
        return Err(ClickHouseError::Query(format!(
            "ClickHouse error: {}",
            body
        )));
    }

    serde_json::from_str(&body).map_err(|e| ClickHouseError::Query(e.to_string()))
}
