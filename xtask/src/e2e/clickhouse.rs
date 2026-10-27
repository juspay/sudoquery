//! ClickHouse access for the dashboard suite: inspects what dashboard-server
//! created for a workspace and queries as that workspace's user.

use anyhow::{Context, bail};

/// Defaults match the compose `clickhouse` service.
const DEFAULT_URL: &str = "http://localhost:8123";
const DEFAULT_USER: &str = "default";
const DEFAULT_PASSWORD: &str = "OeTMIwYfwg7qwa1idy4cO3Is";
/// Kafka as the compose ClickHouse container reaches it.
const DEFAULT_KAFKA_BROKERS: &str = "redpanda:9092";

/// The answer to a query sent as some user: the server's status and body.
pub(crate) struct Answer {
    pub(crate) status: u16,
    pub(crate) body: String,
}

pub(crate) struct ClickHouse {
    client: reqwest::Client,
    url: String,
    user: String,
    password: String,
    kafka_brokers: String,
}

impl ClickHouse {
    /// Connects to `CLICKHOUSE_URL` as `CLICKHOUSE_USER`/`CLICKHOUSE_PASSWORD`
    /// (default: the compose ClickHouse) and checks it answers.
    /// `CLICKHOUSE_KAFKA_BROKERS` is the broker list as ClickHouse sees it.
    pub(crate) async fn connect() -> anyhow::Result<Self> {
        let var = |name: &str, default: &str| {
            std::env::var(name)
                .ok()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| default.to_owned())
        };
        let clickhouse = Self {
            client: reqwest::Client::new(),
            url: var("CLICKHOUSE_URL", DEFAULT_URL),
            user: var("CLICKHOUSE_USER", DEFAULT_USER),
            // An empty password is a valid setting, so only unset means default.
            password: std::env::var("CLICKHOUSE_PASSWORD")
                .unwrap_or_else(|_| DEFAULT_PASSWORD.to_owned()),
            kafka_brokers: var("CLICKHOUSE_KAFKA_BROKERS", DEFAULT_KAFKA_BROKERS),
        };
        clickhouse.query("SELECT 1").await.with_context(|| {
            format!(
                "ClickHouse at {} is unreachable (hint: `cargo xtask setup clickhouse`)",
                clickhouse.url
            )
        })?;
        println!("e2e clickhouse: {}", clickhouse.url);
        Ok(clickhouse)
    }

    pub(crate) fn url(&self) -> &str {
        &self.url
    }

    pub(crate) fn user(&self) -> &str {
        &self.user
    }

    pub(crate) fn password(&self) -> &str {
        &self.password
    }

    pub(crate) fn kafka_brokers(&self) -> &str {
        &self.kafka_brokers
    }

    /// Runs `sql` as the admin user and returns the raw body.
    pub(crate) async fn query(&self, sql: &str) -> anyhow::Result<String> {
        let answer = self.query_as(&self.user, &self.password, None, sql).await?;
        if answer.status != 200 {
            bail!("ClickHouse rejected {sql:?}: {}", answer.body.trim());
        }
        Ok(answer.body)
    }

    /// The lines of `sql`'s TSV result, run as the admin user.
    pub(crate) async fn rows(&self, sql: &str) -> anyhow::Result<Vec<String>> {
        Ok(self.query(sql).await?.lines().map(str::to_owned).collect())
    }

    /// Runs `sql` as `user` against `database`, returning the answer whether
    /// or not ClickHouse accepted it.
    pub(crate) async fn query_as(
        &self,
        user: &str,
        password: &str,
        database: Option<&str>,
        sql: &str,
    ) -> anyhow::Result<Answer> {
        let mut request = self
            .client
            .post(&self.url)
            .basic_auth(user, Some(password))
            .body(sql.to_owned());
        if let Some(database) = database {
            request = request.query(&[("database", database)]);
        }
        let response = request
            .send()
            .await
            .context("request to ClickHouse failed")?;
        Ok(Answer {
            status: response.status().as_u16(),
            body: response.text().await.unwrap_or_default(),
        })
    }
}

/// `value` as a single-quoted ClickHouse string literal.
pub(crate) fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}
