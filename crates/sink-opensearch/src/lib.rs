//! Kafka → OpenSearch sink for canonical events.
//!
//! Consumes the configured topics, writes each event with a `_bulk` `create`
//! using the event's `id` as the document ID, and commits offsets only after
//! OpenSearch or the dead letter queue has acknowledged every record. Delivery
//! is at-least-once; replays can't create duplicates.
//!
//! Settings come from CAC, like the event collector's; see [`cac`].

pub mod cac;
pub mod config;
pub mod health;
mod opensearch;
mod runtime;

use std::sync::Arc;

use tokio_util::sync::CancellationToken;

use cac::Cac;
pub use config::Config;
use health::Health;
use opensearch::{BulkClientError, OpenSearchWriter};
pub use runtime::Drained;
use runtime::{RuntimeConfig, RuntimeError};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    OpenSearch(#[from] BulkClientError),

    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

/// Runs the sink until `shutdown` is cancelled, then writes out what it has
/// buffered within the configured grace period. `config` holds the
/// process-wide settings from `cac`; `cac` is kept to resolve each org's
/// index.
pub async fn run(
    config: Config,
    cac: Cac,
    health: Arc<Health>,
    shutdown: CancellationToken,
) -> Result<Drained, Error> {
    let writer = OpenSearchWriter::new(&config.opensearch, cac, config.batch.max_bytes)?;
    let runtime_config = RuntimeConfig {
        topics: config.kafka.topics,
        group_id: config.kafka.group_id,
        client_config: config.kafka.client_config,
        batch: config.batch,
        retry: config.retry,
        dlq_topic: config.dlq.topic,
        commit_interval: config.commit.interval(),
        shutdown_grace: config.shutdown.grace(),
    };

    Ok(runtime::run(runtime_config, writer, health, shutdown).await?)
}
