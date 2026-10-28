//! Ability cooldown estimation.

/// Effective cooldown given a base cooldown and ability haste (League formula:
/// `base * 100 / (100 + haste)`). Negative haste is treated as zero.
#[must_use]
pub fn effective_cooldown(base_secs: f64, ability_haste: f64) -> f64 {
    base_secs * 100.0 / (100.0 + ability_haste.max(0.0))
}

/// Seconds left before an ability is available again; `0.0` when ready.
#[must_use]
pub fn remaining_cooldown(cast_time_secs: f64, cooldown_secs: f64, now_secs: f64) -> f64 {
    (cast_time_secs + cooldown_secs - now_secs).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_haste_keeps_base() {
        assert!((effective_cooldown(10.0, 0.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn hundred_haste_halves_cooldown() {
        assert!((effective_cooldown(10.0, 100.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn negative_haste_is_ignored() {
        assert!((effective_cooldown(10.0, -50.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn remaining_decreases_then_clamps() {
        assert!((remaining_cooldown(100.0, 10.0, 104.0) - 6.0).abs() < 1e-9);
        assert!(remaining_cooldown(100.0, 10.0, 120.0).abs() < 1e-9);
    }
}
