//! Detection of ability casts from the cooldown digits shown on a HUD icon.
//!
//! When an ability goes on cooldown, white digits (the remaining seconds) appear on
//! its icon and stay until it is ready again. A cast is therefore the rising edge of
//! "digits visible". The input is, for each second of video, the fraction of near-white
//! pixels in the centre of the icon.

/// Fraction of bright pixels above which digits are considered visible.
const ON_THRESHOLD: f32 = 0.03;
/// Fraction below which digits are considered gone.
const OFF_THRESHOLD: f32 = 0.01;
/// Consecutive samples needed to confirm digits appeared (rejects flashes).
const ON_SAMPLES: usize = 2;
/// Consecutive samples needed to confirm digits are gone. The digit count changes
/// while the cooldown runs, so the ratio dips; a single dip must not end the state.
const OFF_SAMPLES: usize = 3;

/// Seconds (indices of `digit_ratio`) at which the digits appeared.
#[must_use]
pub fn detect_cast_starts(digit_ratio: &[f32]) -> Vec<usize> {
    let mut casts = Vec::new();
    let mut on = false;
    let (mut run_on, mut run_off) = (0usize, 0usize);
    for (t, &v) in digit_ratio.iter().enumerate() {
        if on {
            run_off = if v < OFF_THRESHOLD { run_off + 1 } else { 0 };
            if run_off >= OFF_SAMPLES {
                on = false;
                run_off = 0;
                run_on = 0;
            }
        } else {
            run_on = if v >= ON_THRESHOLD { run_on + 1 } else { 0 };
            if run_on >= ON_SAMPLES {
                on = true;
                let start = t + 1 - ON_SAMPLES;
                // Digits already visible when the recording starts: the ability was cast
                // before we began recording, so this is not a cast we can show.
                if start > 0 {
                    casts.push(start);
                }
                run_on = 0;
                run_off = 0;
            }
        }
    }
    casts
}

/// Estimated cast times in video seconds.
///
/// Digits first seen at keyframe second `s` appeared somewhere in `(s - 1, s]`; the
/// middle of that interval halves the worst-case error (0.5 s instead of 1 s).
#[must_use]
pub fn detect_casts(digit_ratio: &[f32]) -> Vec<f64> {
    detect_cast_starts(digit_ratio)
        .into_iter()
        .map(|s| s as f64 - 0.5)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimated_cast_time_is_the_middle_of_the_second_before_the_first_sighting() {
        let s: Vec<f32> = std::iter::repeat_n(0.0, 10)
            .chain(std::iter::repeat_n(0.08, 20))
            .collect();
        assert_eq!(detect_casts(&s), [9.5]);
    }

    fn series(parts: &[(usize, f32)]) -> Vec<f32> {
        parts
            .iter()
            .flat_map(|&(n, v)| std::iter::repeat_n(v, n))
            .collect()
    }

    #[test]
    fn no_digits_means_no_cast() {
        assert!(detect_cast_starts(&series(&[(100, 0.0)])).is_empty());
    }

    #[test]
    fn digits_appearing_is_a_cast_at_the_first_confirmed_sample() {
        let s = series(&[(10, 0.0), (30, 0.08)]);
        assert_eq!(detect_cast_starts(&s), [10]);
    }

    #[test]
    fn a_cast_is_reported_once_per_cooldown_even_if_the_ratio_fluctuates() {
        // 0.12, 0.05, 0.07, 0.04 ... as the number of digits changes
        let s = series(&[
            (5, 0.0),
            (1, 0.12),
            (1, 0.05),
            (1, 0.07),
            (1, 0.04),
            (20, 0.06),
        ]);
        assert_eq!(detect_cast_starts(&s), [5]);
    }

    #[test]
    fn a_brief_dip_does_not_end_the_cooldown_state() {
        let s = series(&[(3, 0.0), (10, 0.08), (2, 0.0), (10, 0.08)]);
        assert_eq!(detect_cast_starts(&s), [3]);
    }

    #[test]
    fn a_second_cast_is_detected_after_the_ability_is_ready_again() {
        let s = series(&[(3, 0.0), (10, 0.08), (5, 0.0), (10, 0.08)]);
        assert_eq!(detect_cast_starts(&s), [3, 18]);
    }

    #[test]
    fn a_single_bright_sample_is_ignored() {
        let s = series(&[(10, 0.0), (1, 0.5), (10, 0.0)]);
        assert!(detect_cast_starts(&s).is_empty());
    }

    #[test]
    fn a_cooldown_already_running_when_recording_starts_is_not_a_cast() {
        let s = series(&[(10, 0.07), (6, 0.0), (10, 0.07)]);
        assert_eq!(detect_cast_starts(&s), [16]);
    }
}
