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
mod tests;
