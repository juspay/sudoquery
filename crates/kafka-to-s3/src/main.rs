//! kafka-to-s3 archiver: canonical events from Kafka to zstd JSONL on S3.

mod buffer;
mod config;
mod consumer;
mod engine;
mod flusher;
mod object;
mod routing;

use crate::config::Config;
use crate::consumer::{ConsumerError, EventConsumer, KafkaSource};
use crate::engine::{ShutdownSignal, WindowPolicy};
use crate::flusher::{ArchiveSink, FlushError, Flusher};
use std::process::ExitCode;
use tokio::signal::unix::{SignalKind, signal};

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing();
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => {
            tracing::error!(%error, "invalid configuration; exiting");
            return ExitCode::FAILURE;
        }
    };
    match run(config).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(%error, "archiver failed; exiting");
            ExitCode::FAILURE
        }
    }
}

/// Boot or runtime failure. A flush failure exits 1 so the container restarts
/// and replays from the last committed offset (D13).
#[derive(Debug, thiserror::Error)]
enum RunError {
    #[error(transparent)]
    Flush(#[from] FlushError),
    #[error(transparent)]
    Consumer(#[from] ConsumerError),
    #[error("failed to register shutdown signal handler: {0}")]
    Signal(#[from] std::io::Error),
}

async fn run(config: Config) -> Result<(), RunError> {
    let policy = WindowPolicy {
        max_events: config.batch_max_events,
        interval: config.flush_interval,
    };
    let consumer = EventConsumer::new(&config)?;
    tracing::info!(
        topics = ?config.kafka_topics,
        group = %config.kafka_group_id,
        bucket = %config.s3_bucket,
        max_events = config.batch_max_events,
        interval_secs = config.flush_interval.as_secs(),
        "kafka-to-s3 archiver started"
    );
    let source = KafkaSource::new(&consumer);
    let sink = ArchiveSink::new(Flusher::new(&config).await, &consumer);
    let shutdown = spawn_signal_handler()?;
    engine::run_engine(source, sink, policy, shutdown).await?;
    tracing::info!("shutdown complete");
    Ok(())
}

/// D15: SIGTERM or SIGINT stops consuming and triggers the final flush.
fn spawn_signal_handler() -> Result<ShutdownSignal, std::io::Error> {
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
    let mut sigterm = signal(SignalKind::terminate())?;
    let mut sigint = signal(SignalKind::interrupt())?;
    tokio::spawn(async move {
        tokio::select! {
            _ = sigterm.recv() => {},
            _ = sigint.recv() => {},
        }
        tracing::info!("shutdown signal received; flushing remaining events");
        // Send fails only when the engine has already finished.
        let _ = shutdown_tx.send(true);
    });
    Ok(shutdown_rx)
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}
