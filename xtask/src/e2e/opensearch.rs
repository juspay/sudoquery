//! OpenSearch access for the dashboard suite: checks the data streams
//! dashboard-server created for workspaces, and deletes them afterwards.

use anyhow::{Context, bail, ensure};

/// Default matches the compose `opensearch` service.
const DEFAULT_URL: &str = "http://localhost:9200";
/// The index template that makes `events-*` names data streams, installed
/// by the compose `opensearch-init` service.
const INDEX_TEMPLATE: &str = "events";

pub(crate) struct OpenSearch {
    client: reqwest::Client,
    url: String,
}

impl OpenSearch {
    /// Connects to `OPENSEARCH_URL` (default: the compose OpenSearch) and
    /// checks the `events` index template is installed.
    pub(crate) async fn connect() -> anyhow::Result<Self> {
        let url = std::env::var("OPENSEARCH_URL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_URL.to_owned());
        let opensearch = Self {
            client: reqwest::Client::new(),
            url: url.trim_end_matches('/').to_owned(),
        };
        let template = opensearch
            .status(&format!("_index_template/{INDEX_TEMPLATE}"))
            .await
            .with_context(|| {
                format!(
                    "OpenSearch at {} is unreachable (hint: `cargo xtask setup opensearch opensearch-init`)",
                    opensearch.url
                )
            })?;
        ensure!(
            template == 200,
            "OpenSearch at {} has no `{INDEX_TEMPLATE}` index template (hint: `cargo xtask setup opensearch-init`)",
            opensearch.url
        );
        println!("e2e opensearch: {}", opensearch.url);
        Ok(opensearch)
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) async fn data_stream_exists(&self, name: &str) -> anyhow::Result<bool> {
        match self.status(&format!("_data_stream/{name}")).await? {
            200 => Ok(true),
            404 => Ok(false),
            status => bail!("OpenSearch answered HTTP {status} for data stream {name}"),
        }
    }

    /// Deletes every data stream whose name starts with `prefix`.
    pub(crate) async fn delete_data_streams(&self, prefix: &str) -> anyhow::Result<()> {
        let response = self
            .client
            .delete(format!("{}/_data_stream/{prefix}*", self.url))
            .send()
            .await
            .context("request to OpenSearch failed")?;
        let status = response.status();
        ensure!(
            status.is_success() || status == reqwest::StatusCode::NOT_FOUND,
            "deleting data streams {prefix}*: HTTP {status}: {}",
            response.text().await.unwrap_or_default()
        );
        Ok(())
    }

    async fn status(&self, path: &str) -> anyhow::Result<u16> {
        let response = self
            .client
            .get(format!("{}/{path}", self.url))
            .send()
            .await
            .context("request to OpenSearch failed")?;
        Ok(response.status().as_u16())
    }
}
