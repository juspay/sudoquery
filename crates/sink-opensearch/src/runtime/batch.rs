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
mod tests {
    use std::time::Duration;

    use super::*;

    fn limits(max_docs: usize, max_bytes: usize) -> BatchConfig {
        BatchConfig {
            max_docs,
            max_bytes,
            linger_ms: 1000,
            max_in_flight: 1,
        }
    }

    fn entry(offset: i64, size: usize, received: Instant) -> Entry<()> {
        Entry {
            offset,
            key: None,
            payload: None,
            doc: Ok(()),
            size,
            received,
        }
    }

    fn offsets(batch: &[Entry<()>]) -> Vec<i64> {
        batch.iter().map(|entry| entry.offset).collect()
    }

    #[test]
    fn becomes_full_on_doc_count() {
        let limits = limits(2, 1000);
        let mut buffer = PartitionBuffer::default();
        let now = Instant::now();

        buffer.push(entry(0, 10, now));
        assert!(!buffer.is_full(&limits));
        buffer.push(entry(1, 10, now));

        assert!(buffer.is_full(&limits));
        assert!(buffer.is_ready(now, &limits));
    }

    #[test]
    fn becomes_full_on_bytes() {
        let limits = limits(100, 50);
        let mut buffer = PartitionBuffer::default();
        let now = Instant::now();

        buffer.push(entry(0, 30, now));
        buffer.push(entry(1, 30, now));

        assert!(buffer.is_full(&limits));
    }

    #[test]
    fn becomes_ready_after_linger() {
        let limits = limits(100, 1000);
        let mut buffer = PartitionBuffer::default();
        let start = Instant::now();
        buffer.push(entry(0, 10, start));

        assert!(!buffer.is_ready(start + Duration::from_millis(999), &limits));
        assert!(buffer.is_ready(start + Duration::from_millis(1000), &limits));
    }

    #[test]
    fn empty_buffer_is_never_ready() {
        let buffer = PartitionBuffer::<()>::default();

        assert!(!buffer.is_ready(Instant::now() + Duration::from_secs(60), &limits(1, 1)));
    }

    #[test]
    fn take_batch_respects_doc_limit_and_keeps_order() {
        let limits = limits(2, 1000);
        let mut buffer = PartitionBuffer::default();
        let now = Instant::now();
        for offset in 0..5 {
            buffer.push(entry(offset, 10, now));
        }

        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![0, 1]);
        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![2, 3]);
        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![4]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_batch_respects_byte_limit() {
        let limits = limits(100, 50);
        let mut buffer = PartitionBuffer::default();
        let now = Instant::now();
        buffer.push(entry(0, 20, now));
        buffer.push(entry(1, 20, now));
        buffer.push(entry(2, 20, now));

        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![0, 1]);
        assert!(!buffer.is_full(&limits));
        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![2]);
    }

    #[test]
    fn take_batch_always_takes_one_record() {
        let limits = limits(100, 50);
        let mut buffer = PartitionBuffer::default();
        buffer.push(entry(0, 80, Instant::now()));

        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![0]);
    }

    #[test]
    fn rejected_records_take_no_request_bytes() {
        let limits = limits(100, 50);
        let mut buffer = PartitionBuffer::default();
        let now = Instant::now();
        buffer.push(entry(0, 50, now));
        buffer.push(Entry {
            doc: Err(Rejection::new("decode", "not JSON")),
            ..entry(1, 0, now)
        });

        assert_eq!(offsets(&buffer.take_batch(&limits)), vec![0, 1]);
    }
}
