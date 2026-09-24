//! Batch-cycle engine: wait-for-n-or-timeout windowing (D1, D2) and the
//! `Source`/`Sink` seams that make the loop testable without Kafka or S3.

use crate::buffer::{Batch, Buffer, Consumed};
use crate::flusher::FlushError;
use crate::routing::{Classification, classify};
use std::future::Future;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::{Instant, sleep_until};

/// Window policy: `max_events` is D1's `n`, `interval` is the window length.
#[derive(Debug, Clone, Copy)]
pub struct WindowPolicy {
    pub max_events: usize,
    pub interval: Duration,
}

/// Message source seam; `None` ends the engine gracefully.
pub trait Source: Send {
    fn recv(&mut self) -> impl Future<Output = Option<Consumed>> + Send;
}

/// Flush destination seam: one call per batch cycle.
pub trait Sink: Send {
    /// Flush a cycle: upload everything, then best-effort commit (D8, D14).
    fn flush(&mut self, batch: Batch) -> impl Future<Output = Result<(), FlushError>> + Send;

    /// Final flush on shutdown: upload everything, then synchronous commit (D15).
    fn flush_final(&mut self, batch: Batch) -> impl Future<Output = Result<(), FlushError>> + Send {
        self.flush(batch)
    }
}

/// Shutdown signal fed by the SIGTERM/SIGINT handler task (D15).
pub type ShutdownSignal = watch::Receiver<bool>;

/// Run the archiver loop until shutdown or source end.
///
/// D1 semantics: wait until `n` events are buffered (total across all
/// topics/partitions) or `interval` has elapsed since the window started,
/// whichever comes first. An n-trigger dequeues exactly `n` events FIFO — surplus
/// stays buffered, and surplus ≥ `n` fires the next cycle immediately. A timeout
/// dequeues everything buffered (< `n`). Quarantined events count toward `n`.
/// The window tumbles: the deadline resets after every cycle, including
/// zero-event no-op windows (D2). One cycle = uploads + commit; commits come from
/// the dequeued batch only, after all uploads succeeded (D8).
///
/// # Errors
/// Propagates [`FlushError`] when any flush fails (the binary exits 1, per D13).
pub async fn run_engine<S: Source, F: Sink>(
    mut source: S,
    mut sink: F,
    policy: WindowPolicy,
    mut shutdown: ShutdownSignal,
) -> Result<(), FlushError> {
    let mut buffer = Buffer::new();
    let mut deadline = Instant::now() + policy.interval;
    loop {
        if buffer.len() >= policy.max_events {
            let batch = buffer.take_n(policy.max_events);
            sink.flush(batch).await?;
            deadline = Instant::now() + policy.interval;
            continue;
        }
        tokio::select! {
            consumed = source.recv() => match consumed {
                Some(msg) => match classify(&msg.payload) {
                    Classification::Routed(key) => buffer.push(key, msg),
                    Classification::Quarantined(_) => buffer.push_quarantine(msg),
                },
                None => break,
            },
            _ = sleep_until(deadline) => {
                if !buffer.is_empty() {
                    sink.flush(buffer.take_all()).await?;
                }
                deadline = Instant::now() + policy.interval;
            }
            _ = shutdown.changed() => break,
        }
    }
    if !buffer.is_empty() {
        sink.flush_final(buffer.take_all()).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
