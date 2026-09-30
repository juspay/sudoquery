use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ClickHouseError {
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Query error: {0}")]
    Query(String),
}

pub async fn create_project_user(
    admin_url: &str,
    admin_user: &str,
    admin_password: &str,
    project_password: &str,
    project_id: Uuid,
) -> Result<(), ClickHouseError> {
    let client = Client::new();
    let project_id_str = project_id.to_string();

    let create_query = format!(
        "CREATE USER '{}' IDENTIFIED WITH sha256_password BY '{}'",
        project_id_str, project_password
    );
    execute_admin_query(
        &client,
        admin_url,
        admin_user,
        admin_password,
        &create_query,
    )
    .await?;

    let grant_query = format!("GRANT project_user TO '{}'", project_id_str);
    execute_admin_query(&client, admin_url, admin_user, admin_password, &grant_query).await?;

    Ok(())
}

pub async fn drop_project_user(
    admin_url: &str,
    admin_user: &str,
    admin_password: &str,
    project_id: Uuid,
) -> Result<(), ClickHouseError> {
    let client = Client::new();
    let project_id_str = project_id.to_string();

    let drop_query = format!("DROP USER IF EXISTS '{}'", project_id_str);
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

pub async fn execute_project_query(
    clickhouse_url: &str,
    project_id: Uuid,
    project_password: &str,
    query: &str,
) -> Result<Value, ClickHouseError> {
    let client = Client::new();
    let project_id_str = project_id.to_string();
    let formatted_query = format!("{} FORMAT JSON", query.trim());

    println!("Executing query for project {}: {}", project_id_str, query);

    let url = format!("{}?database=default", clickhouse_url);

    let response = client
        .post(&url)
        .basic_auth(&project_id_str, Some(project_password))
        .body(formatted_query)
        .send()
        .await
        .map_err(|e| ClickHouseError::Http(e.to_string()))?;

    let status = response.status();
    println!("Received response with status: {}", status);
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        tracing::error!("ClickHouse error for project {}: {}", project_id, body);
        return Err(ClickHouseError::Query(format!(
            "ClickHouse error: {}",
            body
        )));
    }

    serde_json::from_str(&body).map_err(|e| ClickHouseError::Query(e.to_string()))
}
