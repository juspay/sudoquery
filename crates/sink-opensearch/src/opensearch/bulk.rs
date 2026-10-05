//! HTTP client for the `_bulk` API.

use bytes::Bytes;
use reqwest::Url;
use reqwest::header::CONTENT_TYPE;

use super::classify::{ItemClass, RequestClass, classify_item, classify_request};
use super::response::{self, ItemResult};
use crate::config::OpenSearchConfig;
use crate::runtime::{ItemOutcome, Rejection, WriteError};

/// Longest response excerpt kept in an error message.
const MAX_ERROR_BODY: usize = 512;

pub struct BulkClient {
    http: reqwest::Client,
    endpoint: Url,
    credentials: Option<(String, String)>,
}

impl BulkClient {
    pub fn new(config: &OpenSearchConfig) -> Result<Self, BulkClientError> {
        let mut base =
            Url::parse(&config.url).map_err(|error| BulkClientError::Url(error.to_string()))?;
        // Keep any path prefix: `http://host/prefix` + `_bulk` must not
        // replace `prefix`.
        if !base.path().ends_with('/') {
            let path = format!("{}/", base.path());
            base.set_path(&path);
        }

        let http = reqwest::Client::builder()
            .timeout(config.request_timeout())
            .build()?;

        Ok(Self {
            http,
            endpoint: base
                .join("_bulk")
                .map_err(|error| BulkClientError::Url(error.to_string()))?,
            credentials: config.username.clone().zip(
                config
                    .password
                    .as_ref()
                    .map(|secret| secret.expose().to_owned()),
            ),
        })
    }

    /// Sends one `_bulk` request holding `expected` documents and returns one
    /// outcome per document, in order.
    pub async fn send(&self, body: Bytes, expected: usize) -> Result<Vec<ItemOutcome>, WriteError> {
        let mut request = self
            .http
            .post(self.endpoint.clone())
            .header(CONTENT_TYPE, "application/x-ndjson")
            .body(body);
        if let Some((username, password)) = &self.credentials {
            request = request.basic_auth(username, Some(password));
        }

        let response = request.send().await.map_err(|error| WriteError {
            reason: format!("bulk request failed: {}", error.without_url()),
            alert: false,
        })?;

        let status = response.status().as_u16();
        if let RequestClass::Retry { alert } = classify_request(status) {
            let body = response.text().await.unwrap_or_default();
            return Err(WriteError {
                reason: format!("bulk request returned HTTP {status}: {}", excerpt(&body)),
                alert,
            });
        }

        let body = response.bytes().await.map_err(|error| WriteError {
            reason: format!("failed to read bulk response: {}", error.without_url()),
            alert: false,
        })?;
        let items = response::parse(&body, expected).map_err(|reason| WriteError {
            reason,
            alert: false,
        })?;

        Ok(items.into_iter().map(outcome).collect())
    }
}

fn outcome(item: ItemResult) -> ItemOutcome {
    let describe = || {
        let error = item.error.as_deref().unwrap_or("no error details");
        format!("item status {}: {error}", item.status)
    };
    match classify_item(item.status) {
        ItemClass::Done => ItemOutcome::Done,
        ItemClass::AlreadyWritten => ItemOutcome::AlreadyWritten,
        ItemClass::Retry { alert } => ItemOutcome::Retry {
            reason: describe(),
            alert,
        },
        ItemClass::Reject => {
            ItemOutcome::Reject(Rejection::new("rejected", describe()).with_status(item.status))
        }
    }
}

fn excerpt(body: &str) -> &str {
    match body.char_indices().nth(MAX_ERROR_BODY) {
        Some((end, _)) => &body[..end],
        None => body,
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BulkClientError {
    #[error("invalid OpenSearch URL: {0}")]
    Url(String),

    #[error("failed to build the HTTP client: {0}")]
    Http(#[from] reqwest::Error),
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use wiremock::matchers::{basic_auth, body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::config::{IndexTemplate, Secret};

    const BODY: &str = "{\"create\":{\"_index\":\"events\",\"_id\":\"a\"}}\n{\"n\":1}\n";

    fn config(url: &str) -> OpenSearchConfig {
        OpenSearchConfig {
            url: url.to_owned(),
            index: IndexTemplate::parse("events").unwrap(),
            request_timeout_ms: 1000,
            username: None,
            password: None,
        }
    }

    fn items(statuses: &[(u16, Option<&str>)]) -> serde_json::Value {
        let items: Vec<_> = statuses
            .iter()
            .map(|(status, error)| match error {
                Some(kind) => json!({"create": {"status": status, "error": {"type": kind, "reason": "details"}}}),
                None => json!({"create": {"status": status}}),
            })
            .collect();
        json!({"took": 1, "errors": true, "items": items})
    }

    async fn server_responding(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    async fn send(client: &BulkClient, expected: usize) -> Result<Vec<ItemOutcome>, WriteError> {
        client
            .send(Bytes::from_static(BODY.as_bytes()), expected)
            .await
    }

    #[tokio::test]
    async fn posts_ndjson_to_the_bulk_endpoint() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(header("content-type", "application/x-ndjson"))
            .and(body_string(BODY))
            .respond_with(ResponseTemplate::new(200).set_body_json(items(&[(201, None)])))
            .expect(1)
            .mount(&server)
            .await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let outcomes = send(&client, 1).await.unwrap();

        assert!(matches!(outcomes.as_slice(), [ItemOutcome::Done]));
    }

    #[tokio::test]
    async fn keeps_a_path_prefix_in_the_url() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/search/_bulk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(items(&[(201, None)])))
            .expect(1)
            .mount(&server)
            .await;
        let client = BulkClient::new(&config(&format!("{}/search", server.uri()))).unwrap();

        assert!(send(&client, 1).await.is_ok());
    }

    #[tokio::test]
    async fn sends_basic_auth_when_configured() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/_bulk"))
            .and(basic_auth("sink", "hunter2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(items(&[(201, None)])))
            .expect(1)
            .mount(&server)
            .await;
        let mut config = config(&server.uri());
        config.username = Some("sink".into());
        config.password = Some(Secret::new("hunter2"));
        let client = BulkClient::new(&config).unwrap();

        assert!(send(&client, 1).await.is_ok());
    }

    #[tokio::test]
    async fn classifies_mixed_item_results() {
        let server = server_responding(ResponseTemplate::new(200).set_body_json(items(&[
            (201, None),
            (409, Some("version_conflict_engine_exception")),
            (429, Some("es_rejected_execution_exception")),
            (400, Some("mapper_parsing_exception")),
            (404, Some("index_not_found_exception")),
            (403, Some("cluster_block_exception")),
        ])))
        .await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let outcomes = send(&client, 6).await.unwrap();

        assert!(matches!(outcomes[0], ItemOutcome::Done));
        assert!(matches!(outcomes[1], ItemOutcome::AlreadyWritten));
        assert!(matches!(
            outcomes[2],
            ItemOutcome::Retry { alert: false, .. }
        ));
        match &outcomes[3] {
            ItemOutcome::Reject(rejection) => {
                assert_eq!(rejection.class, "rejected");
                assert_eq!(rejection.status, Some(400));
                assert!(rejection.reason.contains("mapper_parsing_exception"));
            }
            other => panic!("expected a rejection, got {other:?}"),
        }
        assert!(matches!(
            outcomes[4],
            ItemOutcome::Retry { alert: true, .. }
        ));
        // A disk-full or read-only block must not send good data to the DLQ.
        assert!(matches!(
            outcomes[5],
            ItemOutcome::Retry { alert: true, .. }
        ));
    }

    #[tokio::test]
    async fn throttling_is_a_quiet_retry() {
        let server =
            server_responding(ResponseTemplate::new(429).set_body_string("too many requests"))
                .await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let error = send(&client, 1).await.unwrap_err();

        assert!(!error.alert);
        assert!(error.reason.contains("HTTP 429"));
    }

    #[tokio::test]
    async fn bad_credentials_are_a_retry_with_an_alert() {
        let server =
            server_responding(ResponseTemplate::new(401).set_body_string("unauthorized")).await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let error = send(&client, 1).await.unwrap_err();

        assert!(error.alert);
        assert!(error.reason.contains("HTTP 401: unauthorized"));
    }

    #[tokio::test]
    async fn timeouts_are_retried() {
        let server = server_responding(
            ResponseTemplate::new(200)
                .set_body_json(items(&[(201, None)]))
                .set_delay(Duration::from_secs(3)),
        )
        .await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let error = send(&client, 1).await.unwrap_err();

        assert!(!error.alert);
        assert!(error.reason.starts_with("bulk request failed"));
    }

    #[tokio::test]
    async fn connection_errors_are_retried() {
        let client = BulkClient::new(&config("http://127.0.0.1:9")).unwrap();

        let error = send(&client, 1).await.unwrap_err();

        assert!(!error.alert);
    }

    #[tokio::test]
    async fn item_count_mismatch_is_retried() {
        let server =
            server_responding(ResponseTemplate::new(200).set_body_json(items(&[(201, None)])))
                .await;
        let client = BulkClient::new(&config(&server.uri())).unwrap();

        let error = send(&client, 2).await.unwrap_err();

        assert!(error.reason.contains("1 items for 2 documents"));
    }

    #[test]
    fn excerpt_cuts_long_bodies_on_a_char_boundary() {
        let body = "é".repeat(MAX_ERROR_BODY + 10);

        assert_eq!(excerpt(&body).chars().count(), MAX_ERROR_BODY);
        assert_eq!(excerpt("short"), "short");
    }
}
