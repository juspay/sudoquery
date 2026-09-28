//! Consumer callbacks: keeps the offset tracker in step with rebalances and
//! commit confirmations.
//!
//! librdkafka runs these on the thread polling the consumer, inside
//! `StreamConsumer::recv`, while the event loop is suspended. They only touch
//! the tracker, so they never wait on the event loop.

use std::sync::{Arc, Mutex};

use rdkafka::consumer::{BaseConsumer, CommitMode, Consumer, ConsumerContext, Rebalance};
use rdkafka::error::KafkaResult;
use rdkafka::{ClientContext, Offset, TopicPartitionList};
use tracing::{error, info, warn};

use super::offsets::{OffsetTracker, TopicPartition, lock};

pub struct SinkContext {
    tracker: Arc<Mutex<OffsetTracker>>,
}

impl SinkContext {
    pub fn new(tracker: Arc<Mutex<OffsetTracker>>) -> Self {
        Self { tracker }
    }
}

impl ClientContext for SinkContext {}

impl ConsumerContext for SinkContext {
    /// On revoke, commits what is already written and forgets the partitions.
    /// Batches still in flight are abandoned: the new owner replays them, and
    /// document IDs make the replay harmless.
    fn pre_rebalance(&self, consumer: &BaseConsumer<Self>, rebalance: &Rebalance<'_>) {
        match rebalance {
            Rebalance::Revoke(revoked) => {
                let mut commits = TopicPartitionList::new();
                {
                    let mut tracker = lock(&self.tracker);
                    for element in revoked.elements() {
                        let tp = TopicPartition::new(element.topic(), element.partition());
                        if let Some(offset) = tracker.revoke(&tp)
                            && let Err(error) = commits.add_partition_offset(
                                element.topic(),
                                element.partition(),
                                Offset::Offset(offset),
                            )
                        {
                            warn!(%error, topic = %tp.topic, partition = tp.partition, "invalid offset on revoke");
                        }
                    }
                }

                info!(partitions = revoked.count(), "partitions revoked");
                if commits.count() > 0 {
                    match consumer.commit(&commits, CommitMode::Sync) {
                        Ok(()) => {
                            info!(partitions = commits.count(), "committed offsets on revoke")
                        }
                        Err(error) => warn!(
                            %error,
                            "offset commit on revoke failed; the new owner replays from the last committed offset"
                        ),
                    }
                }
            }
            Rebalance::Assign(_) => {}
            Rebalance::Error(error) => error!(%error, "rebalance failed"),
        }
    }

    fn post_rebalance(&self, _consumer: &BaseConsumer<Self>, rebalance: &Rebalance<'_>) {
        if let Rebalance::Assign(assigned) = rebalance {
            let mut tracker = lock(&self.tracker);
            for element in assigned.elements() {
                tracker.assign(TopicPartition::new(element.topic(), element.partition()));
            }
            info!(partitions = assigned.count(), "partitions assigned");
        }
    }

    fn commit_callback(&self, result: KafkaResult<()>, offsets: &TopicPartitionList) {
        match result {
            Ok(()) => {
                let mut tracker = lock(&self.tracker);
                for element in offsets.elements() {
                    if let Offset::Offset(offset) = element.offset() {
                        tracker.mark_committed(
                            &TopicPartition::new(element.topic(), element.partition()),
                            offset,
                        );
                    }
                }
            }
            Err(error) => warn!(%error, "offset commit failed; retrying on the next commit tick"),
        }
    }
}
