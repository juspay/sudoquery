//! FIFO buffer with exact-n dequeue (D1) and commit-map derivation.

use crate::routing::GroupKey;
use std::collections::{HashMap, VecDeque};

/// A consumed Kafka message pending buffering.
#[derive(Debug, Clone)]
pub struct Consumed {
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub payload: Vec<u8>,
}

/// Routing outcome stored with a buffered entry.
#[derive(Debug, Clone)]
pub enum Origin {
    Routed(GroupKey),
    Quarantined,
}

/// One buffered event: routing outcome + Kafka coordinates + raw payload (D7).
#[derive(Debug, Clone)]
pub struct Entry {
    pub origin: Origin,
    pub topic: String,
    pub partition: i32,
    pub offset: i64,
    pub raw: Vec<u8>,
}

/// Next offset to commit per `(topic, partition)` — max offset + 1.
pub type CommitMap = HashMap<(String, i32), i64>;

/// A dequeued batch cycle: grouped entries, quarantine entries, and the offsets
/// safe to commit once every upload succeeds (D8).
#[derive(Debug, Default)]
pub struct Batch {
    pub groups: Vec<(GroupKey, Vec<Entry>)>,
    pub quarantine: Vec<Entry>,
    pub commits: CommitMap,
}

/// FIFO buffer of consumed events. Quarantined events count toward `n` (D1).
#[derive(Debug, Default)]
pub struct Buffer {
    queue: VecDeque<Entry>,
}

impl Buffer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Total buffered events, routed and quarantined alike (D1: both count toward `n`).
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Buffer a routed event.
    pub fn push(&mut self, key: GroupKey, msg: Consumed) {
        self.queue.push_back(Entry {
            origin: Origin::Routed(key),
            topic: msg.topic,
            partition: msg.partition,
            offset: msg.offset,
            raw: msg.payload,
        });
    }

    /// Buffer an unroutable event (D12): archived to quarantine, offset committed past.
    pub fn push_quarantine(&mut self, msg: Consumed) {
        self.queue.push_back(Entry {
            origin: Origin::Quarantined,
            topic: msg.topic,
            partition: msg.partition,
            offset: msg.offset,
            raw: msg.payload,
        });
    }

    /// Dequeue exactly `n` events FIFO (D1); surplus stays buffered. If fewer than
    /// `n` are buffered, everything is dequeued.
    pub fn take_n(&mut self, n: usize) -> Batch {
        self.drain(n.min(self.queue.len()))
    }

    /// Dequeue every buffered event (timeout and shutdown paths).
    pub fn take_all(&mut self) -> Batch {
        self.drain(self.queue.len())
    }

    fn drain(&mut self, count: usize) -> Batch {
        let mut batch = Batch::default();
        let mut group_index: HashMap<GroupKey, usize> = HashMap::new();
        for _ in 0..count {
            let Some(entry) = self.queue.pop_front() else {
                break;
            };
            batch.record_commit(&entry);
            let group_slot = match &entry.origin {
                Origin::Routed(key) => Some(match group_index.get(key) {
                    Some(&index) => index,
                    None => {
                        batch.groups.push((key.clone(), Vec::new()));
                        let index = batch.groups.len() - 1;
                        group_index.insert(key.clone(), index);
                        index
                    }
                }),
                Origin::Quarantined => None,
            };
            match group_slot {
                Some(index) => batch.groups[index].1.push(entry),
                None => batch.quarantine.push(entry),
            }
        }
        batch
    }
}

impl Batch {
    /// Track max offset + 1 per `(topic, partition)`, from dequeued entries only:
    /// unflushed surplus offsets must never be committed (D8 at-least-once).
    fn record_commit(&mut self, entry: &Entry) {
        let next = entry.offset.saturating_add(1);
        let slot = self
            .commits
            .entry((entry.topic.clone(), entry.partition))
            .or_insert(next);
        if next > *slot {
            *slot = next;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(tenant: &str, workspace: &str, dt: &str, hour: u32) -> GroupKey {
        GroupKey {
            tenant: tenant.to_string(),
            workspace: workspace.to_string(),
            dt: dt.to_string(),
            hour,
        }
    }

    fn consumed(topic: &str, partition: i32, offset: i64, payload: &str) -> Consumed {
        Consumed {
            topic: topic.to_string(),
            partition,
            offset,
            payload: payload.as_bytes().to_vec(),
        }
    }

    fn payloads(entries: &[Entry]) -> Vec<&[u8]> {
        entries.iter().map(|entry| entry.raw.as_slice()).collect()
    }

    #[test]
    fn groups_entries_by_key_in_first_seen_order() {
        let mut buffer = Buffer::new();
        let key_a = key("t1", "w1", "2026-09-24", 9);
        let key_b = key("t2", "w1", "2026-09-24", 9);
        buffer.push(key_a.clone(), consumed("events", 0, 0, "a1"));
        buffer.push(key_b.clone(), consumed("events", 1, 5, "b1"));
        buffer.push(key_a.clone(), consumed("other", 0, 7, "a2"));

        let batch = buffer.take_all();

        assert_eq!(batch.groups.len(), 2);
        assert_eq!(batch.groups[0].0, key_a);
        assert_eq!(
            payloads(&batch.groups[0].1),
            vec![b"a1".as_slice(), b"a2".as_slice()]
        );
        assert_eq!(batch.groups[1].0, key_b);
        assert_eq!(payloads(&batch.groups[1].1), vec![b"b1".as_slice()]);
    }

    #[test]
    fn take_n_dequeues_exact_fifo_and_keeps_surplus() {
        let mut buffer = Buffer::new();
        let key = key("t", "w", "2026-09-24", 9);
        for i in 0..5 {
            buffer.push(key.clone(), consumed("events", 0, i, &format!("e{i}")));
        }

        let first = buffer.take_n(2);
        assert_eq!(
            payloads(&first.groups[0].1),
            vec![b"e0".as_slice(), b"e1".as_slice()]
        );
        assert_eq!(buffer.len(), 3);

        let second = buffer.take_n(2);
        assert_eq!(
            payloads(&second.groups[0].1),
            vec![b"e2".as_slice(), b"e3".as_slice()]
        );
        assert_eq!(buffer.len(), 1);

        let rest = buffer.take_all();
        assert_eq!(payloads(&rest.groups[0].1), vec![b"e4".as_slice()]);
        assert!(buffer.is_empty());
    }

    #[test]
    fn take_n_beyond_len_takes_everything() {
        let mut buffer = Buffer::new();
        buffer.push(
            key("t", "w", "2026-09-24", 9),
            consumed("events", 0, 0, "e0"),
        );
        buffer.push(
            key("t", "w", "2026-09-24", 9),
            consumed("events", 0, 1, "e1"),
        );

        let batch = buffer.take_n(10);
        assert_eq!(batch.groups[0].1.len(), 2);
        assert!(buffer.is_empty());
    }

    #[test]
    fn commit_map_uses_max_plus_one_from_dequeued_batch_only() {
        let mut buffer = Buffer::new();
        let key = key("t", "w", "2026-09-24", 9);
        buffer.push(key.clone(), consumed("events", 0, 0, "e0"));
        buffer.push(key.clone(), consumed("events", 1, 10, "e1"));
        buffer.push(key.clone(), consumed("events", 0, 1, "e2"));
        buffer.push(key.clone(), consumed("events", 1, 11, "e3"));
        buffer.push(key.clone(), consumed("events", 0, 2, "e4"));

        // Dequeue the first three FIFO: (0,0), (1,10), (0,1). The remaining
        // (1,11) and (0,2) are surplus and must NOT be committed.
        let batch = buffer.take_n(3);
        assert_eq!(
            batch.commits,
            HashMap::from([
                (("events".to_string(), 0), 2),
                (("events".to_string(), 1), 11),
            ])
        );

        let rest = buffer.take_all();
        assert_eq!(
            rest.commits,
            HashMap::from([
                (("events".to_string(), 0), 3),
                (("events".to_string(), 1), 12),
            ])
        );
    }

    #[test]
    fn quarantine_entries_count_toward_len_and_commits() {
        let mut buffer = Buffer::new();
        buffer.push(
            key("t", "w", "2026-09-24", 9),
            consumed("events", 0, 0, "ok"),
        );
        buffer.push_quarantine(consumed("events", 0, 1, "poison"));
        assert_eq!(buffer.len(), 2);

        let batch = buffer.take_all();
        assert_eq!(batch.groups.len(), 1);
        assert_eq!(batch.quarantine.len(), 1);
        assert_eq!(batch.quarantine[0].raw, b"poison");
        assert_eq!(
            batch.commits,
            HashMap::from([(("events".to_string(), 0), 2)])
        );
    }

    #[test]
    fn take_all_on_empty_buffer_is_a_noop() {
        let mut buffer = Buffer::new();
        let batch = buffer.take_all();
        assert!(batch.groups.is_empty());
        assert!(batch.quarantine.is_empty());
        assert!(batch.commits.is_empty());
    }

    #[test]
    fn raw_payload_bytes_are_preserved_verbatim() {
        let payload = br#"{"id":"e","trailing whitespace":"kept"}  "#;
        let mut buffer = Buffer::new();
        buffer.push(
            key("t", "w", "2026-09-24", 9),
            Consumed {
                topic: "events".to_string(),
                partition: 3,
                offset: 42,
                payload: payload.to_vec(),
            },
        );

        let batch = buffer.take_all();
        assert_eq!(batch.groups[0].1[0].raw, payload);
    }
}
