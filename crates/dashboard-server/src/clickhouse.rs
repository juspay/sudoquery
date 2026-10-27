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

/// ClickHouse error names that describe server/infra problems rather than the user's query
const INTERNAL_ERROR_NAMES: &[&str] = &[
    "(AUTHENTICATION_FAILED)",
    "(UNKNOWN_USER)",
    "(REQUIRED_PASSWORD)",
    "(WRONG_PASSWORD)",
    "(NETWORK_ERROR)",
    "(SOCKET_TIMEOUT)",
    "(ALL_CONNECTION_TRIES_FAILED)",
    "(DNS_ERROR)",
];

impl ClickHouseError {
    /// Message safe to return to clients for user-written queries: ClickHouse's own
    /// error line (code, message, error name) without version, stack trace or infra errors.
    pub fn public_message(&self) -> String {
        const GENERIC: &str = "Query failed";

        let ClickHouseError::Query(body) = self else {
            return GENERIC.to_string();
        };

        let line = body.lines().next().unwrap_or("").trim();
        if !line.starts_with("Code: ") || INTERNAL_ERROR_NAMES.iter().any(|n| line.contains(n)) {
            return GENERIC.to_string();
        }

        let line = match line.find(" (version ") {
            Some(i) => &line[..i],
            None => line,
        };
        line.replace("DB::Exception: ", "")
    }
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
        return Err(ClickHouseError::Query(body));
    }

    serde_json::from_str(&body).map_err(|e| ClickHouseError::Query(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::ClickHouseError;

    #[test]
    fn a_query_error_keeps_the_clickhouse_message_without_the_version() {
        let error = ClickHouseError::Query("Code: 62. DB::Exception: Syntax error: failed at position 8 ('FRM'): FRM x. Expected one of: FROM. (SYNTAX_ERROR) (version 24.3.1.2672 (official build))\n".into());
        assert_eq!(
            error.public_message(),
            "Code: 62. Syntax error: failed at position 8 ('FRM'): FRM x. Expected one of: FROM. (SYNTAX_ERROR)"
        );
    }

    #[test]
    fn infra_and_transport_errors_are_generic() {
        let errors = [
            ClickHouseError::Query("Code: 516. DB::Exception: abc: Authentication failed: password is incorrect. (AUTHENTICATION_FAILED) (version 24.3.1.2672 (official build))".into()),
            ClickHouseError::Query("\"10.0.101.196:42878\": invalid username or password for user \"default\"; query: \"CREATE USER 'x' IDENTIFIED WITH sha256_password BY ''\"\n".into()),
            ClickHouseError::Query("expected value at line 1 column 1".into()),
            ClickHouseError::Http("error sending request for url (http://10.0.0.5:8123/)".into()),
        ];
        for error in errors {
            assert_eq!(error.public_message(), "Query failed", "{error:?}");
        }
    }
}
