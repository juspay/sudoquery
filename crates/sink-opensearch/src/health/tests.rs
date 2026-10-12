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
