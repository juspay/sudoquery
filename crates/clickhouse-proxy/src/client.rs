//! A client for clickhouse-proxy's API (see `server`), for the services
//! that call it: dashboard-server runs its queries and provisions workspaces
//! through it.

use std::time::Duration;

use reqwest::{Method, Url};
use serde_json::{Value, json};

use crate::clickhouse::GENERIC_MESSAGE;

#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    /// The proxy answered with an error; `message` is its `error` field.
    #[error("{message}")]
    Status { status: u16, message: String },
    #[error("clickhouse-proxy is unreachable: {0}")]
    Unreachable(String),
}

impl ProxyError {
    /// Safe to show to whoever asked: the proxy's own message, which it
    /// keeps free of internals, or a generic one when it couldn't be reached.
    pub fn public_message(&self) -> String {
        match self {
            ProxyError::Status { message, .. } => message.clone(),
            ProxyError::Unreachable(_) => GENERIC_MESSAGE.to_owned(),
        }
    }
}

#[derive(Clone)]
pub struct ClickHouseProxy {
    http: reqwest::Client,
    base_url: Url,
    /// Bearer token the proxy requires (`CLICKHOUSE_PROXY_TOKEN`).
    token: Option<String>,
}

impl ClickHouseProxy {
    pub fn new(base_url: &str, token: Option<String>, timeout: Duration) -> Result<Self, String> {
        let base_url = Url::parse(base_url).map_err(|error| format!("{base_url}: {error}"))?;
        if base_url.cannot_be_a_base() {
            return Err(format!("{base_url} cannot be a base URL"));
        }
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            http,
            base_url,
            token,
        })
    }

    /// `{base}/v1/workspaces/{workspace}/{path…}`, each segment escaped.
    fn url(&self, workspace_id: &str, path: &[&str]) -> Url {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("checked in new")
            .pop_if_empty()
            .extend(["v1", "workspaces", workspace_id])
            .extend(path);
        url
    }

    fn request(&self, method: Method, url: Url) -> reqwest::RequestBuilder {
        let request = self.http.request(method, url);
        match &self.token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    /// Sends a request for the workspace and returns the proxy's status and
    /// body, whatever they are, for a caller that relays them as they came.
    pub async fn relay(
        &self,
        method: Method,
        workspace_id: &str,
        path: &[&str],
        query: &[(&str, &str)],
        body: Option<Vec<u8>>,
    ) -> Result<(u16, Vec<u8>), ProxyError> {
        let mut request = self
            .request(method, self.url(workspace_id, path))
            .query(query);
        if let Some(body) = body {
            request = request
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }
        let reply = request
            .send()
            .await
            .map_err(|error| ProxyError::Unreachable(error.without_url().to_string()))?;
        let status = reply.status().as_u16();
        let bytes = reply
            .bytes()
            .await
            .map_err(|error| ProxyError::Unreachable(error.without_url().to_string()))?;
        Ok((status, bytes.to_vec()))
    }

    /// A JSON call: the parsed body of a 2xx answer (`null` when empty),
    /// otherwise the proxy's error.
    async fn call(
        &self,
        method: Method,
        workspace_id: &str,
        path: &[&str],
        query: &[(&str, &str)],
        body: Option<Value>,
    ) -> Result<Value, ProxyError> {
        let body = body.map(|body| body.to_string().into_bytes());
        let (status, bytes) = self.relay(method, workspace_id, path, query, body).await?;
        let parsed: Value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap_or(Value::Null)
        };
        if (200..300).contains(&status) {
            return Ok(parsed);
        }
        let message = parsed["error"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("clickhouse-proxy answered HTTP {status}"));
        Err(ProxyError::Status { status, message })
    }

    /// Runs SQL as the workspace's ClickHouse user; ClickHouse's JSON answer.
    pub async fn query(&self, workspace_id: &str, sql: &str) -> Result<Value, ProxyError> {
        self.call(
            Method::POST,
            workspace_id,
            &["query"],
            &[],
            Some(json!({ "query": sql })),
        )
        .await
    }

    pub async fn event_names(&self, workspace_id: &str) -> Result<Vec<String>, ProxyError> {
        let names = self
            .call(Method::GET, workspace_id, &["events"], &[], None)
            .await?;
        Ok(strings(&names))
    }

    /// Events that occurred today in `timezone`.
    pub async fn today_count(&self, workspace_id: &str, timezone: &str) -> Result<u64, ProxyError> {
        let answer = self
            .call(
                Method::GET,
                workspace_id,
                &["events", "today-count"],
                &[("timezone", timezone)],
                None,
            )
            .await?;
        Ok(answer["count"].as_u64().unwrap_or(0))
    }

    pub async fn event_properties(
        &self,
        workspace_id: &str,
        event_name: &str,
    ) -> Result<Vec<String>, ProxyError> {
        let keys = self
            .call(
                Method::GET,
                workspace_id,
                &["events", event_name, "properties"],
                &[],
                None,
            )
            .await?;
        Ok(strings(&keys))
    }

    /// Creates the workspace's ClickHouse database, fed from `topic`.
    pub async fn create_database(&self, workspace_id: &str, topic: &str) -> Result<(), ProxyError> {
        self.call(
            Method::PUT,
            workspace_id,
            &["database"],
            &[],
            Some(json!({ "topic": topic })),
        )
        .await
        .map(drop)
    }

    pub async fn drop_database(&self, workspace_id: &str) -> Result<(), ProxyError> {
        self.call(Method::DELETE, workspace_id, &["database"], &[], None)
            .await
            .map(drop)
    }

    pub async fn create_user(&self, workspace_id: &str) -> Result<(), ProxyError> {
        self.call(Method::PUT, workspace_id, &["user"], &[], None)
            .await
            .map(drop)
    }

    pub async fn drop_user(&self, workspace_id: &str) -> Result<(), ProxyError> {
        self.call(Method::DELETE, workspace_id, &["user"], &[], None)
            .await
            .map(drop)
    }

    /// Whether the proxy can reach ClickHouse (its `/health`).
    pub async fn clickhouse_connected(&self) -> bool {
        let mut url = self.base_url.clone();
        url.path_segments_mut()
            .expect("checked in new")
            .pop_if_empty()
            .push("health");
        match self.request(Method::GET, url).send().await {
            Ok(reply) => reply
                .json::<Value>()
                .await
                .is_ok_and(|body| body["clickhouse"] == "connected"),
            Err(_) => false,
        }
    }
}

fn strings(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn client(server: &MockServer, token: Option<&str>) -> ClickHouseProxy {
        ClickHouseProxy::new(
            &format!("{}/base/", server.uri()),
            token.map(str::to_owned),
            Duration::from_secs(2),
        )
        .unwrap()
    }

    #[tokio::test]
    async fn a_query_goes_to_the_workspace_with_the_token() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/base/v1/workspaces/acme-shop/query"))
            .and(header("authorization", "Bearer secret"))
            .and(body_json(json!({ "query": "SELECT 1" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "rows": 1 })))
            .expect(1)
            .mount(&server)
            .await;

        let answer = client(&server, Some("secret"))
            .query("acme-shop", "SELECT 1")
            .await
            .unwrap();
        assert_eq!(answer, json!({ "rows": 1 }));
    }

    #[tokio::test]
    async fn the_proxys_error_message_is_kept() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(
                ResponseTemplate::new(400)
                    .set_body_json(json!({ "error": "Code: 62. Syntax error" })),
            )
            .mount(&server)
            .await;

        let error = client(&server, None)
            .query("acme-shop", "SELEC")
            .await
            .unwrap_err();
        assert!(matches!(error, ProxyError::Status { status: 400, .. }));
        assert_eq!(error.public_message(), "Code: 62. Syntax error");
    }

    #[tokio::test]
    async fn event_names_and_segments_are_escaped() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path(
                "/base/v1/workspaces/acme-shop/events/a%2Fb%20c/properties",
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!(["plan"])))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/base/v1/workspaces/acme-shop/events/today-count"))
            .and(query_param("timezone", "Asia/Kolkata"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "count": 7 })))
            .mount(&server)
            .await;

        let proxy = client(&server, None);
        assert_eq!(
            proxy.event_properties("acme-shop", "a/b c").await.unwrap(),
            vec!["plan"]
        );
        assert_eq!(
            proxy
                .today_count("acme-shop", "Asia/Kolkata")
                .await
                .unwrap(),
            7
        );
    }

    #[tokio::test]
    async fn provisioning_sends_the_topic_and_accepts_no_content() {
        let server = MockServer::start().await;
        Mock::given(method("PUT"))
            .and(path("/base/v1/workspaces/acme-shop/database"))
            .and(body_json(json!({ "topic": "events.acme.acme-shop" })))
            .respond_with(ResponseTemplate::new(204))
            .expect(1)
            .mount(&server)
            .await;

        client(&server, None)
            .create_database("acme-shop", "events.acme.acme-shop")
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn an_unreachable_proxy_has_a_generic_message() {
        let proxy =
            ClickHouseProxy::new("http://127.0.0.1:9", None, Duration::from_secs(2)).unwrap();
        let error = proxy.query("acme-shop", "SELECT 1").await.unwrap_err();
        assert!(matches!(error, ProxyError::Unreachable(_)));
        assert_eq!(error.public_message(), "Query failed");
        assert!(!proxy.clickhouse_connected().await);
    }
}
