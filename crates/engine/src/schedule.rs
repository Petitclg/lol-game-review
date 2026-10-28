//! When to look for application updates.

use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateOutcome {
    /// Already on the latest release.
    UpToDate,
    /// A newer release exists but a game is running, so installing is postponed.
    DeferredInGame,
    /// The check or the installation failed (network, signature, ...).
    Failed,
}

const FAILURE_BACKOFF_SECS: [u64; 3] = [30, 120, 600];
const DEFERRED_SECS: u64 = 600;
const RECHECK_SECS: u64 = 6 * 3600;

/// Delay before the next check. `consecutive_failures` counts failures in a row,
/// including the one that just happened.
#[must_use]
pub fn next_check_delay(outcome: UpdateOutcome, consecutive_failures: usize) -> Duration {
    let secs = match outcome {
        UpdateOutcome::UpToDate => RECHECK_SECS,
        UpdateOutcome::DeferredInGame => DEFERRED_SECS,
        UpdateOutcome::Failed => FAILURE_BACKOFF_SECS
            .get(consecutive_failures.saturating_sub(1))
            .copied()
            .unwrap_or(RECHECK_SECS),
    };
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_to_date_rechecks_every_six_hours() {
        assert_eq!(
            next_check_delay(UpdateOutcome::UpToDate, 0),
            Duration::from_secs(6 * 3600)
        );
    }

    #[test]
    fn deferred_by_a_game_retries_soon() {
        assert_eq!(
            next_check_delay(UpdateOutcome::DeferredInGame, 0),
            Duration::from_secs(600)
        );
    }

    #[test]
    fn failures_back_off_then_settle_on_the_long_interval() {
        let d = |n| next_check_delay(UpdateOutcome::Failed, n).as_secs();
        assert_eq!([d(1), d(2), d(3)], [30, 120, 600]);
        assert_eq!(d(4), 6 * 3600);
        assert_eq!(d(0), 30);
    }
}
