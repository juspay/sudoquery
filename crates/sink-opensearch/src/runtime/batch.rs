//! Per-partition buffer of records waiting to be written.

use std::collections::VecDeque;

use bytes::Bytes;
use tokio::time::Instant;

use super::Rejection;
use crate::config::BatchConfig;

/// One consumed record. Every entry ends up written, or in the dead letter
/// queue with its original key and payload.
#[derive(Debug)]
pub struct Entry<D> {
    pub offset: i64,
    pub key: Option<Bytes>,
    pub payload: Option<Bytes>,
    pub doc: Result<D, Rejection>,
    /// Bytes the document adds to a write request; 0 for rejected records.
    pub size: usize,
    pub received: Instant,
}

#[derive(Debug)]
pub struct PartitionBuffer<D> {
    entries: VecDeque<Entry<D>>,
    bytes: usize,
}

impl<D> Default for PartitionBuffer<D> {
    fn default() -> Self {
        Self {
            entries: VecDeque::new(),
            bytes: 0,
        }
    }
}

impl<D> PartitionBuffer<D> {
    pub fn push(&mut self, entry: Entry<D>) {
        self.bytes += entry.size;
        self.entries.push_back(entry);
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn is_full(&self, limits: &BatchConfig) -> bool {
        self.entries.len() >= limits.max_docs || self.bytes >= limits.max_bytes
    }

    /// Full, or the oldest record has waited `linger`.
    pub fn is_ready(&self, now: Instant, limits: &BatchConfig) -> bool {
        self.is_full(limits)
            || self
                .entries
                .front()
                .is_some_and(|oldest| now.duration_since(oldest.received) >= limits.linger())
    }

    /// Takes the oldest records, up to `max_docs` records and `max_bytes`
    /// bytes. Always takes at least one record if any are buffered.
    pub fn take_batch(&mut self, limits: &BatchConfig) -> Vec<Entry<D>> {
        let mut batch = Vec::new();
        let mut bytes = 0;

        while let Some(next) = self.entries.front() {
            let over_limit = batch.len() >= limits.max_docs || bytes + next.size > limits.max_bytes;
            if !batch.is_empty() && over_limit {
                break;
            }
            let Some(entry) = self.entries.pop_front() else {
                break;
            };
            bytes += entry.size;
            batch.push(entry);
        }

        self.bytes -= bytes;
        batch
    }
}

#[cfg(test)]
mod tests;
