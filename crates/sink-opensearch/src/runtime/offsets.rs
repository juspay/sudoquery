//! The only source of offsets that may be committed.
//!
//! Each partition has at most one batch in flight and batches complete in
//! order, so a partition's commit position is simply the offset after the last
//! record of its last completed batch.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct TopicPartition {
    pub topic: String,
    pub partition: i32,
}

impl TopicPartition {
    pub fn new(topic: &str, partition: i32) -> Self {
        Self {
            topic: topic.to_owned(),
            partition,
        }
    }
}

#[derive(Debug, Default)]
pub struct OffsetTracker {
    last_epoch: u64,
    partitions: HashMap<TopicPartition, Tracked>,
}

#[derive(Debug)]
struct Tracked {
    /// Distinguishes this assignment from earlier ones of the same partition,
    /// so results from a previous owner's batches are ignored.
    epoch: u64,
    /// The next offset to read, i.e. the value Kafka should store.
    next_commit: Option<i64>,
    committed: Option<i64>,
}

impl Tracked {
    fn uncommitted(&self) -> Option<i64> {
        self.next_commit
            .filter(|next| Some(*next) != self.committed)
    }
}

impl OffsetTracker {
    /// Starts tracking a newly assigned partition and returns its epoch.
    pub fn assign(&mut self, tp: TopicPartition) -> u64 {
        self.last_epoch += 1;
        self.partitions.insert(
            tp,
            Tracked {
                epoch: self.last_epoch,
                next_commit: None,
                committed: None,
            },
        );
        self.last_epoch
    }

    /// Stops tracking a revoked partition. Returns the offset that still needs
    /// committing, if any.
    pub fn revoke(&mut self, tp: &TopicPartition) -> Option<i64> {
        self.partitions
            .remove(tp)
            .and_then(|tracked| tracked.uncommitted())
    }

    pub fn epoch(&self, tp: &TopicPartition) -> Option<u64> {
        self.partitions.get(tp).map(|tracked| tracked.epoch)
    }

    /// Records that every record up to and including `last_offset` is written
    /// or dead-lettered. Ignored if the partition was reassigned since.
    pub fn complete(&mut self, tp: &TopicPartition, epoch: u64, last_offset: i64) -> bool {
        match self.partitions.get_mut(tp) {
            Some(tracked) if tracked.epoch == epoch => {
                let next = last_offset + 1;
                tracked.next_commit = Some(tracked.next_commit.map_or(next, |n| n.max(next)));
                true
            }
            _ => false,
        }
    }

    /// Offsets that have advanced past what Kafka has confirmed.
    pub fn pending_commits(&self) -> Vec<(TopicPartition, i64)> {
        self.partitions
            .iter()
            .filter_map(|(tp, tracked)| tracked.uncommitted().map(|offset| (tp.clone(), offset)))
            .collect()
    }

    pub fn mark_committed(&mut self, tp: &TopicPartition, offset: i64) {
        if let Some(tracked) = self.partitions.get_mut(tp)
            && tracked.committed.is_none_or(|committed| offset > committed)
        {
            tracked.committed = Some(offset);
        }
    }
}

/// Locks the tracker even if a panicking thread poisoned it; its state is
/// always consistent between calls.
pub fn lock(tracker: &Mutex<OffsetTracker>) -> MutexGuard<'_, OffsetTracker> {
    tracker.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
