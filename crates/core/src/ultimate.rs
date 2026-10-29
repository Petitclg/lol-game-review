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

/// Index of the ultimate icon in a row of four ability icons (Q, W, E, R).
pub const ULTIMATE_ICON: usize = 3;

/// A calibration needs at least this many cooldown episodes on Q, W and E combined.
/// Those abilities are cast constantly, so a correct geometry shows dozens per game.
const MIN_CALIBRATION_EPISODES: usize = 5;
/// The best geometry must beat the runner-up by this factor, otherwise it is ambiguous.
const MIN_CALIBRATION_MARGIN: usize = 2;

/// Number of cooldown episodes seen on Q, W and E for one candidate geometry.
fn basic_ability_episodes(rows: &[[f32; 4]]) -> usize {
    (0..ULTIMATE_ICON)
        .map(|icon| {
            let series: Vec<f32> = rows.iter().map(|r| r[icon]).collect();
            detect_cast_starts(&series).len()
        })
        .sum()
}

/// Calibration score of every candidate geometry (for diagnostics).
#[must_use]
pub fn calibration_scores(candidates: &[Vec<[f32; 4]>]) -> Vec<usize> {
    candidates
        .iter()
        .map(|rows| basic_ability_episodes(rows))
        .collect()
}

/// Candidates scoring at least this fraction of the best belong to the same plateau:
/// neighbouring scales still see most of the digits, so the true scale lies within it.
const PLATEAU_RATIO: f64 = 0.7;

/// Picks, among candidate geometries, the one whose icons show cooldown digits.
///
/// `candidates[i]` is the per-second digit ratio of the four icons for geometry `i`.
/// A wrong geometry reads background pixels and almost never shows long, clean cooldown
/// episodes, so the right one stands out. Neighbouring scales score nearly as high, so
/// the answer is the centre of the plateau of high scores, and the margin is measured
/// against candidates *outside* that neighbourhood. Returns `None` when no geometry
/// stands out (HUD hidden, unusual layout, recording too short, or two distant
/// candidates that both look plausible).
#[must_use]
pub fn calibrate(candidates: &[Vec<[f32; 4]>]) -> Option<usize> {
    let scores = calibration_scores(candidates);
    let (best, &best_score) = scores
        .iter()
        .enumerate()
        .max_by_key(|&(i, &s)| (s, std::cmp::Reverse(i)))?;
    if best_score < MIN_CALIBRATION_EPISODES {
        return None;
    }
    let on_plateau = |s: usize| s as f64 >= PLATEAU_RATIO * best_score as f64;
    let mut lo = best;
    while lo > 0 && on_plateau(scores[lo - 1]) {
        lo -= 1;
    }
    let mut hi = best;
    while hi + 1 < scores.len() && on_plateau(scores[hi + 1]) {
        hi += 1;
    }
    let runner_up = scores
        .iter()
        .enumerate()
        .filter(|&(i, _)| i + 1 < lo || i > hi + 1)
        .map(|(_, &s)| s)
        .max()
        .unwrap_or(0);
    if best_score < MIN_CALIBRATION_MARGIN * runner_up {
        return None;
    }
    let (weighted, total) = (lo..=hi).fold((0.0, 0.0), |(w, t), i| {
        (w + i as f64 * scores[i] as f64, t + scores[i] as f64)
    });
    Some((weighted / total).round() as usize)
}

/// Result of analysing the HUD of a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct UltimateDetection {
    /// Index of the geometry (candidate) that was recognised.
    pub candidate: usize,
    /// Estimated cast times, in video seconds.
    pub casts_video_secs: Vec<f64>,
}

/// Recognises the HUD geometry, then finds the ultimate casts. `None` when the HUD
/// could not be recognised, so that no clip is produced from noise.
#[must_use]
pub fn detect_ultimates(candidates: &[Vec<[f32; 4]>]) -> Option<UltimateDetection> {
    let candidate = calibrate(candidates)?;
    let ultimate: Vec<f32> = candidates[candidate]
        .iter()
        .map(|r| r[ULTIMATE_ICON])
        .collect();
    Some(UltimateDetection {
        candidate,
        casts_video_secs: detect_casts(&ultimate),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rows where each listed icon shows digits during the given `(start, len)` spans.
    fn rows(len: usize, spans: &[(usize, usize, usize)]) -> Vec<[f32; 4]> {
        let mut out = vec![[0.0f32; 4]; len];
        for &(icon, start, n) in spans {
            for row in out.iter_mut().skip(start).take(n) {
                row[icon] = 0.08;
            }
        }
        out
    }

    /// A geometry showing `per_icon` cooldown episodes on each of Q, W and E.
    fn plausible(per_icon: usize) -> Vec<[f32; 4]> {
        let mut spans = Vec::new();
        for icon in 0..3 {
            for k in 0..per_icon {
                spans.push((icon, 10 + k * 40 + icon * 3, 12));
            }
        }
        rows(600, &spans)
    }

    #[test]
    fn the_geometry_with_clear_cooldown_episodes_is_chosen() {
        let candidates = vec![plausible(0), plausible(0), plausible(6), plausible(0)];
        assert_eq!(calibrate(&candidates), Some(2));
    }

    #[test]
    fn a_weak_winner_is_rejected() {
        // 4 episodes only: below the minimum
        assert_eq!(calibrate(&[plausible(0), plausible(1)]), None);
    }

    #[test]
    fn two_distant_plausible_geometries_are_ambiguous() {
        let candidates = vec![
            plausible(6),
            plausible(0),
            plausible(0),
            plausible(0),
            plausible(5),
        ];
        assert_eq!(calibrate(&candidates), None);
    }

    #[test]
    fn neighbouring_scales_with_similar_scores_pick_the_centre_of_the_plateau() {
        let candidates = vec![
            plausible(0),
            plausible(6),
            plausible(6),
            plausible(6),
            plausible(0),
        ];
        assert_eq!(calibrate(&candidates), Some(2));
    }

    #[test]
    fn a_true_scale_between_two_candidates_is_accepted() {
        let candidates = vec![plausible(0), plausible(6), plausible(5), plausible(0)];
        assert!(calibrate(&candidates).is_some());
    }

    #[test]
    fn a_neighbouring_geometry_with_noise_does_not_hide_the_right_one() {
        let candidates = vec![plausible(1), plausible(8), plausible(2)];
        assert_eq!(calibrate(&candidates), Some(1));
    }

    #[test]
    fn no_candidates_or_no_digits_means_unrecognised() {
        assert_eq!(calibrate(&[]), None);
        assert_eq!(calibrate(&[plausible(0), plausible(0)]), None);
        assert!(detect_ultimates(&[plausible(0)]).is_none());
    }

    #[test]
    fn ultimate_casts_are_read_from_the_recognised_geometry_only() {
        let mut right = plausible(6);
        for row in right.iter_mut().skip(100).take(60) {
            row[ULTIMATE_ICON] = 0.08;
        }
        for row in right.iter_mut().skip(300).take(60) {
            row[ULTIMATE_ICON] = 0.08;
        }
        // the wrong geometry also "sees" an ultimate, which must be ignored
        let mut wrong = plausible(0);
        for row in wrong.iter_mut().skip(50).take(60) {
            row[ULTIMATE_ICON] = 0.08;
        }
        let d = detect_ultimates(&[wrong, right]).unwrap();
        assert_eq!(d.candidate, 1);
        assert_eq!(d.casts_video_secs, [99.5, 299.5]);
    }

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
