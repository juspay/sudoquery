//! `/health` and `/metrics` endpoints.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use metrics_exporter_prometheus::{BuildError, Matcher, PrometheusBuilder, PrometheusHandle};
use serde::Serialize;
use tokio::net::TcpListener;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

/// The event loop is considered stuck if it hasn't ticked for this long.
const STALL_AFTER: Duration = Duration::from_secs(30);

const BULK_DURATION_BUCKETS: &[f64] = &[0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0];

/// Liveness of the event loop, which ticks several times a second.
pub struct Health {
    started: Instant,
    last_tick_ms: AtomicU64,
}

impl Default for Health {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            last_tick_ms: AtomicU64::new(0),
        }
    }
}

impl Health {
    pub fn tick(&self) {
        self.last_tick_ms
            .store(self.elapsed_ms(), Ordering::Relaxed);
    }

    pub fn is_alive(&self) -> bool {
        let idle = self
            .elapsed_ms()
            .saturating_sub(self.last_tick_ms.load(Ordering::Relaxed));
        Duration::from_millis(idle) < STALL_AFTER
    }

    fn elapsed_ms(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

/// Installs the global Prometheus recorder for the `metrics` macros.
pub fn install_metrics() -> Result<PrometheusHandle, BuildError> {
    PrometheusBuilder::new()
        .set_buckets_for_metric(
            Matcher::Full("sink_bulk_duration_seconds".into()),
            BULK_DURATION_BUCKETS,
        )?
        .install_recorder()
}

#[derive(Clone)]
struct AppState {
    health: Arc<Health>,
    metrics: PrometheusHandle,
}

#[derive(Serialize)]
struct HealthBody {
    status: &'static str,
}

/// Serves `/health` and `/metrics` until `shutdown` is cancelled.
pub async fn serve(
    listener: TcpListener,
    health: Arc<Health>,
    metrics: PrometheusHandle,
    shutdown: CancellationToken,
) -> std::io::Result<()> {
    // Without the exporter's own HTTP listener, histogram upkeep is ours to run.
    let upkeep = metrics.clone();
    let upkeep_shutdown = shutdown.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                () = upkeep_shutdown.cancelled() => break,
                _ = interval.tick() => upkeep.run_upkeep(),
            }
        }
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .with_state(AppState { health, metrics });

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown.cancelled_owned())
        .await
}

async fn health_handler(State(state): State<AppState>) -> (StatusCode, Json<HealthBody>) {
    if state.health.is_alive() {
        (StatusCode::OK, Json(HealthBody { status: "ok" }))
    } else {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(HealthBody { status: "stalled" }),
        )
    }
}

async fn metrics_handler(State(state): State<AppState>) -> String {
    state.metrics.render()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_health_is_alive() {
        let health = Health::default();

        health.tick();

        assert!(health.is_alive());
    }

    #[test]
    fn a_loop_that_stopped_ticking_is_stalled() {
        let health = Health {
            started: Instant::now() - STALL_AFTER - Duration::from_secs(1),
            last_tick_ms: AtomicU64::new(0),
        };

        assert!(!health.is_alive());
    }
}
