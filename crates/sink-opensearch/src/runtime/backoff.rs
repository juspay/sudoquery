//! Exponential backoff with full jitter.

use std::time::Duration;

use rand::Rng;

use crate::config::RetryConfig;

#[derive(Clone, Copy, Debug)]
pub struct Backoff {
    initial: Duration,
    max: Duration,
}

impl Backoff {
    pub fn new(initial: Duration, max: Duration) -> Self {
        Self { initial, max }
    }

    pub fn from_config(config: &RetryConfig) -> Self {
        Self::new(config.initial_backoff(), config.max_backoff())
    }

    /// Upper bound for the given attempt (starting at 1):
    /// `min(max, initial * 2^(attempt - 1))`.
    pub fn cap(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31);
        self.initial
            .checked_mul(1 << exponent)
            .map_or(self.max, |delay| delay.min(self.max))
    }

    /// A random delay between zero and the cap.
    pub fn delay(&self, attempt: u32) -> Duration {
        let cap = u64::try_from(self.cap(attempt).as_millis()).unwrap_or(u64::MAX);
        Duration::from_millis(rand::rng().random_range(0..=cap))
    }
}

#[cfg(test)]
mod tests {
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
}
