//! Timing for periodic jobs inside a sampler, such as re-reading the drive list.

use std::time::{Duration, Instant};

/// Says when a periodic job is due: on the first check, then once per period.
#[derive(Debug)]
pub(crate) struct Cadence {
    period: Duration,
    last: Option<Instant>,
}

impl Cadence {
    pub(crate) fn new(period: Duration) -> Self {
        Self { period, last: None }
    }

    /// Whether the job is due at `now`. A `true` answer starts the next period.
    pub(crate) fn due(&mut self, now: Instant) -> bool {
        let due = self
            .last
            .is_none_or(|last| now.saturating_duration_since(last) >= self.period);
        if due {
            self.last = Some(now);
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cadence_is_due_first_then_once_per_period() {
        let start = Instant::now();
        let mut cadence = Cadence::new(Duration::from_secs(10));
        assert!(cadence.due(start));
        assert!(!cadence.due(start + Duration::from_secs(1)));
        assert!(!cadence.due(start + Duration::from_millis(9_999)));
        assert!(cadence.due(start + Duration::from_secs(10)));
        assert!(!cadence.due(start + Duration::from_secs(19)));
        assert!(cadence.due(start + Duration::from_secs(25)));
        assert!(!cadence.due(start + Duration::from_secs(34)));
    }
}
