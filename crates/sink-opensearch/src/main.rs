use std::process::ExitCode;
use std::sync::Arc;

use sink_opensearch::health::{self, Health};
use sink_opensearch::{Config, Drained};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    init_tracing();

    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            error!(%error, "invalid configuration");
            return ExitCode::FAILURE;
        }
    };
    let metrics = match health::install_metrics() {
        Ok(handle) => handle,
        Err(error) => {
            error!(%error, "failed to set up metrics");
            return ExitCode::FAILURE;
        }
    };
    let listener = match TcpListener::bind(config.server.addr).await {
        Ok(listener) => listener,
        Err(error) => {
            error!(%error, addr = %config.server.addr, "failed to bind the health server");
            return ExitCode::FAILURE;
        }
    };

    let health = Arc::new(Health::default());
    let shutdown = CancellationToken::new();
    let server_shutdown = CancellationToken::new();
    tokio::spawn(shutdown_on_signal(shutdown.clone()));
    let server = tokio::spawn(health::serve(
        listener,
        Arc::clone(&health),
        metrics,
        server_shutdown.clone(),
    ));

    let result = sink_opensearch::run(config, health, shutdown).await;

    server_shutdown.cancel();
    match server.await {
        Ok(Err(error)) => warn!(%error, "health server stopped with an error"),
        Err(error) => warn!(%error, "health server task failed"),
        Ok(Ok(())) => {}
    }

    match result {
        Ok(Drained::Complete) => {
            info!("stopped cleanly");
            ExitCode::SUCCESS
        }
        Ok(Drained::Incomplete) => {
            warn!("stopped before every record was written; they will be replayed on restart");
            ExitCode::FAILURE
        }
        Err(error) => {
            error!(%error, "sink failed");
            ExitCode::FAILURE
        }
    }
}

/// JSON logs by default; `LOG_FORMAT=pretty` for local development.
/// `RUST_LOG` sets the filter (default `info`).
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let pretty =
        std::env::var("LOG_FORMAT").is_ok_and(|format| format.eq_ignore_ascii_case("pretty"));

    if pretty {
        tracing_subscriber::fmt().with_env_filter(filter).init();
    } else {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .init();
    }
}

async fn shutdown_on_signal(shutdown: CancellationToken) {
    let terminate = async {
        #[cfg(unix)]
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                error!(%error, "failed to listen for SIGTERM");
                std::future::pending::<()>().await;
            }
        }
        #[cfg(not(unix))]
        std::future::pending::<()>().await;
    };

    tokio::select! {
        result = tokio::signal::ctrl_c() => {
            if let Err(error) = result {
                error!(%error, "failed to listen for Ctrl-C");
                std::future::pending::<()>().await;
            }
        }
        () = terminate => {}
    }

    info!("shutdown requested");
    shutdown.cancel();
}
