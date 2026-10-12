use super::*;

fn tp(partition: i32) -> TopicPartition {
    TopicPartition::new("events", partition)
}

#[test]
fn nothing_is_pending_before_a_batch_completes() {
    let mut tracker = OffsetTracker::default();
    tracker.assign(tp(0));

    assert!(tracker.pending_commits().is_empty());
    assert_eq!(tracker.revoke(&tp(0)), None);
}

#[test]
fn completed_batch_commits_the_next_offset() {
    let mut tracker = OffsetTracker::default();
    let epoch = tracker.assign(tp(0));

    assert!(tracker.complete(&tp(0), epoch, 41));

    assert_eq!(tracker.pending_commits(), vec![(tp(0), 42)]);
}

#[test]
fn committed_offsets_are_no_longer_pending() {
    let mut tracker = OffsetTracker::default();
    let epoch = tracker.assign(tp(0));
    tracker.complete(&tp(0), epoch, 41);

    tracker.mark_committed(&tp(0), 42);

    assert!(tracker.pending_commits().is_empty());

    tracker.complete(&tp(0), epoch, 99);
    assert_eq!(tracker.pending_commits(), vec![(tp(0), 100)]);
}

#[test]
fn a_late_commit_confirmation_does_not_move_backwards() {
    let mut tracker = OffsetTracker::default();
    let epoch = tracker.assign(tp(0));
    tracker.complete(&tp(0), epoch, 99);
    tracker.mark_committed(&tp(0), 100);

    tracker.mark_committed(&tp(0), 42);

    assert!(tracker.pending_commits().is_empty());
}

#[test]
fn results_from_an_earlier_assignment_are_ignored() {
    let mut tracker = OffsetTracker::default();
    let old_epoch = tracker.assign(tp(0));
    tracker.revoke(&tp(0));
    let new_epoch = tracker.assign(tp(0));

    assert_ne!(old_epoch, new_epoch);
    assert!(!tracker.complete(&tp(0), old_epoch, 500));
    assert!(tracker.pending_commits().is_empty());
}

#[test]
fn results_for_a_revoked_partition_are_ignored() {
    let mut tracker = OffsetTracker::default();
    let epoch = tracker.assign(tp(0));
    tracker.revoke(&tp(0));

    assert!(!tracker.complete(&tp(0), epoch, 10));
    assert_eq!(tracker.epoch(&tp(0)), None);
}

#[test]
fn revoke_returns_only_uncommitted_offsets() {
    let mut tracker = OffsetTracker::default();
    let first = tracker.assign(tp(0));
    let second = tracker.assign(tp(1));
    tracker.complete(&tp(0), first, 9);
    tracker.complete(&tp(1), second, 19);
    tracker.mark_committed(&tp(1), 20);

    assert_eq!(tracker.revoke(&tp(0)), Some(10));
    assert_eq!(tracker.revoke(&tp(1)), None);
}

#[test]
fn partitions_are_tracked_independently() {
    let mut tracker = OffsetTracker::default();
    let first = tracker.assign(tp(0));
    let second = tracker.assign(tp(1));
    tracker.complete(&tp(0), first, 5);
    tracker.complete(&tp(1), second, 7);

    let mut pending = tracker.pending_commits();
    pending.sort_by_key(|(tp, _)| tp.partition);

    assert_eq!(pending, vec![(tp(0), 6), (tp(1), 8)]);
}
