//! Rates from cumulative counters, such as the bytes a disk has read since boot.

/// Units per second between two readings of a cumulative counter. A counter that went back
/// (a device that was replaced) reads as idle.
pub(crate) fn rate(before: u64, after: u64, elapsed_secs: f64) -> u64 {
    if elapsed_secs <= 0.0 {
        return 0;
    }
    (after.saturating_sub(before) as f64 / elapsed_secs) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_is_the_delta_over_the_interval_and_never_negative() {
        assert_eq!(rate(1_000, 3_000, 0.5), 4_000);
        assert_eq!(rate(3_000, 1_000, 1.0), 0);
        assert_eq!(rate(0, 5_000, 0.0), 0);
    }
}
