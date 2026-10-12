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
