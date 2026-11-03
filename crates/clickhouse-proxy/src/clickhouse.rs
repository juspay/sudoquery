//! ClickHouse over HTTP: queries run as a workspace's own user in its own
//! database, and DDL runs as the admin user.

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

/// What [`ClickHouseError::public_message`] says when it can say nothing more.
pub const GENERIC_MESSAGE: &str = "Query failed";

impl ClickHouseError {
    /// Message safe to return to clients for user-written queries: ClickHouse's own
    /// error line (code, message, error name) without version, stack trace or infra errors.
    pub fn public_message(&self) -> String {
        let ClickHouseError::Query(body) = self else {
            return GENERIC_MESSAGE.to_string();
        };

        let line = body.lines().next().unwrap_or("").trim();
        if !line.starts_with("Code: ") || INTERNAL_ERROR_NAMES.iter().any(|n| line.contains(n)) {
            return GENERIC_MESSAGE.to_string();
        }

        let line = match line.find(" (version ") {
            Some(i) => &line[..i],
            None => line,
        };
        line.replace("DB::Exception: ", "")
    }

    /// Whether the client's query is at fault, rather than ClickHouse or the
    /// way to it.
    pub fn is_query_error(&self) -> bool {
        self.public_message() != GENERIC_MESSAGE
    }
}

/// Where ClickHouse is and how to sign in to it.
#[derive(Clone, Debug)]
pub struct ClickHouseConfig {
    /// Queries run here, as each workspace's user (`CLICKHOUSE_URL`).
    pub url: String,
    /// DDL runs here (`CLICKHOUSE_ADMIN_URL`).
    pub admin_url: String,
    pub admin_user: String,
    pub admin_password: String,
    /// Every workspace user's password (`CLICKHOUSE_WORKSPACE_PASSWORD`).
    pub workspace_password: String,
}

#[derive(Clone)]
pub struct ClickHouse {
    http: Client,
    config: ClickHouseConfig,
}

impl ClickHouse {
    pub fn new(config: ClickHouseConfig) -> Self {
        Self {
            http: Client::new(),
            config,
        }
    }

    pub fn workspace_password(&self) -> &str {
        &self.config.workspace_password
    }

    /// `text` without the workspace password, which a ClickHouse error may
    /// echo (a failed `CREATE USER`, say).
    pub fn redact(&self, text: &str) -> String {
        let password = &self.config.workspace_password;
        if password.is_empty() {
            text.to_owned()
        } else {
            text.replace(password.as_str(), "[REDACTED]")
        }
    }

    /// Runs a statement as the admin user; returns ClickHouse's answer as text.
    pub async fn admin(&self, statement: &str) -> Result<String, ClickHouseError> {
        let response = self
            .http
            .post(&self.config.admin_url)
            .basic_auth(&self.config.admin_user, Some(&self.config.admin_password))
            .body(statement.to_string())
            .send()
            .await
            .map_err(|e| ClickHouseError::Http(e.without_url().to_string()))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(ClickHouseError::Query(body));
        }

        Ok(body)
    }

    /// Runs `query` as the workspace's user, in its database, and returns
    /// ClickHouse's JSON answer (`meta`, `data`, `rows`, `statistics`).
    ///
    /// `params` are ClickHouse query parameters: `{name:String}` in the
    /// query reads the value of `name`, so values never have to be pasted
    /// into the SQL.
    pub async fn query(
        &self,
        workspace_id: &str,
        query: &str,
        params: &[(&str, &str)],
    ) -> Result<Value, ClickHouseError> {
        let query = query.trim().trim_end_matches(';').trim_end();
        let formatted_query = format!("{query} FORMAT JSON");

        tracing::debug!(workspace_id, query, "executing workspace query");

        // Every workspace has its own database, named after the workspace
        // (provisioning.rs), so unqualified table names resolve to its tables.
        let mut url_params = vec![("database".to_owned(), workspace_id.to_owned())];
        url_params.extend(
            params
                .iter()
                .map(|(name, value)| (format!("param_{name}"), (*value).to_owned())),
        );

        let response = self
            .http
            .post(&self.config.url)
            .query(&url_params)
            .basic_auth(workspace_id, Some(&self.config.workspace_password))
            .body(formatted_query)
            .send()
            .await
            .map_err(|e| ClickHouseError::Http(e.without_url().to_string()))?;

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            tracing::error!(workspace_id, "ClickHouse error: {}", self.redact(&body));
            return Err(ClickHouseError::Query(body));
        }

        serde_json::from_str(&body).map_err(|e| ClickHouseError::Query(e.to_string()))
    }
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
        assert!(error.is_query_error());
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
            assert!(!error.is_query_error());
        }
    }
}
