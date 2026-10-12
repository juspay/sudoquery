use super::*;

fn backoff() -> Backoff {
    Backoff::new(Duration::from_millis(100), Duration::from_secs(30))
}

#[test]
fn cap_doubles_per_attempt() {
    let backoff = backoff();

    assert_eq!(backoff.cap(1), Duration::from_millis(100));
    assert_eq!(backoff.cap(2), Duration::from_millis(200));
    assert_eq!(backoff.cap(5), Duration::from_millis(1600));
}

#[test]
fn cap_stops_at_max() {
    let backoff = backoff();

    assert_eq!(backoff.cap(10), Duration::from_secs(30));
    assert_eq!(backoff.cap(u32::MAX), Duration::from_secs(30));
}

#[test]
fn delay_stays_within_cap() {
    let backoff = backoff();

    for attempt in 1..20 {
        for _ in 0..50 {
            assert!(backoff.delay(attempt) <= backoff.cap(attempt));
        }
    }
}
