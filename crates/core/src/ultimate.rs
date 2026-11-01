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

/// Index of the ultimate icon in a row (Q, W, E, R, then the two summoner spells).
pub const ULTIMATE_ICON: usize = 3;
/// Columns of a row holding the brightness of the two summoner-spell icons.
pub const SUMMONER_BRIGHTNESS: [usize; 2] = [6, 7];
/// Values read per second of video: a digit ratio for each of the six icons, then the
/// brightness of the two summoner-spell icons.
pub const HUD_COLUMNS: usize = 8;
/// What is read from the HUD for one second of video.
pub type HudRow = [f32; HUD_COLUMNS];

/// A summoner-spell icon is covered by the cooldown overlay when it is darker than this
/// fraction of its ready brightness.
const COOLDOWN_BRIGHTNESS: f32 = 0.92;
/// The ready brightness is this percentile of the icon's brightness over the game.
const READY_PERCENTILE: f64 = 0.9;
/// The darkest level is this percentile: low enough to see a single cooldown, high
/// enough to ignore a stray frame.
const DARK_PERCENTILE: f64 = 0.01;
/// The icon must have been at least this much darker (relative to the ready brightness)
/// at some point, otherwise it never went on cooldown.
const MIN_CONTRAST: f32 = 0.08;
/// Consecutive dark samples that confirm a cooldown, and bright ones that end it.
const DARK_SAMPLES: usize = 2;
const BRIGHT_SAMPLES: usize = 3;
/// A summoner-spell cooldown darkens the icon for at least this many seconds. Shorter
/// darkness is something else: the greyed HUD of a death, a screen effect, a spell whose
/// cooldown is too short to matter.
const MIN_SUMMONER_COOLDOWN_SECS: usize = 90;

/// Estimated cast times, in video seconds, of a summoner spell, from the brightness of
/// its icon for each second of video.
///
/// The ready icon of some spells (Flash) is itself full of bright streaks, so white
/// cooldown digits cannot be told from the art; the dark overlay that covers the icon
/// during the cooldown can. `from` is the second at which the game starts (the loading
/// screen before it is not the HUD); a cooldown already running then is not a cast.
#[must_use]
pub fn detect_summoner_casts(brightness: &[f32], from: usize) -> Vec<f64> {
    let series = brightness.get(from..).unwrap_or_default();
    if series.len() < MIN_SUMMONER_COOLDOWN_SECS {
        return Vec::new();
    }
    let mut sorted = series.to_vec();
    sorted.sort_by(f32::total_cmp);
    let at = |q: f64| sorted[((sorted.len() - 1) as f64 * q) as usize];
    let (ready, low) = (at(READY_PERCENTILE), at(DARK_PERCENTILE));
    if ready <= 0.0 || ready - low < MIN_CONTRAST * ready {
        return Vec::new();
    }
    let dark = |v: f32| v < COOLDOWN_BRIGHTNESS * ready;

    let mut casts = Vec::new();
    let mut episode_start: Option<usize> = None;
    let (mut run_dark, mut run_bright) = (0usize, 0usize);
    for (t, &v) in series.iter().enumerate() {
        match episode_start {
            None => {
                run_dark = if dark(v) { run_dark + 1 } else { 0 };
                if run_dark >= DARK_SAMPLES {
                    episode_start = Some(t + 1 - DARK_SAMPLES);
                    run_bright = 0;
                }
            }
            Some(start) => {
                run_bright = if dark(v) { 0 } else { run_bright + 1 };
                if run_bright >= BRIGHT_SAMPLES {
                    let end = t + 1 - BRIGHT_SAMPLES;
                    push_cooldown(&mut casts, start, end, from);
                    episode_start = None;
                    run_dark = 0;
                }
            }
        }
    }
    if let Some(start) = episode_start {
        push_cooldown(&mut casts, start, series.len(), from);
    }
    casts
}

fn push_cooldown(casts: &mut Vec<f64>, start: usize, end: usize, from: usize) {
    // A cooldown already running when the game starts was not cast on the recording.
    if start > 0 && end - start >= MIN_SUMMONER_COOLDOWN_SECS {
        casts.push((from + start) as f64 - 0.5);
    }
}

/// A calibration needs at least this many cooldown episodes on Q, W and E combined.
/// Those abilities are cast constantly, so a correct geometry shows dozens per game.
const MIN_CALIBRATION_EPISODES: usize = 5;
/// The best geometry must beat the runner-up by this factor, otherwise it is ambiguous.
const MIN_CALIBRATION_MARGIN: f64 = 2.0;

/// Number of cooldown episodes seen on Q, W and E for one candidate geometry.
fn basic_ability_episodes(rows: &[HudRow]) -> usize {
    (0..ULTIMATE_ICON)
        .map(|icon| {
            let series: Vec<f32> = rows.iter().map(|r| r[icon]).collect();
            detect_cast_starts(&series).len()
        })
        .sum()
}

/// Total brightness of the cooldown digits seen on Q, W and E: the sum of the digit
/// ratios over every second where digits are visible.
///
/// This is the calibration score rather than the episode count: a geometry that is only
/// close to the true one still catches the edge of large digits and counts almost as
/// many episodes, but it sees far less of them. The true geometry centres the digits in
/// each cell, so it collects by far the most.
///
/// A smaller candidate scale crops a smaller area, which magnifies the digits: they
/// cover `1 / scale²` times more of the cell. The mass is multiplied by `scale²` so that
/// the magnification does not favour small scales.
fn basic_ability_mass(rows: &[HudRow], scale: f64) -> f64 {
    let raw: f64 = rows
        .iter()
        .flat_map(|r| r[..ULTIMATE_ICON].iter())
        .filter(|&&v| v >= ON_THRESHOLD)
        .map(|&v| f64::from(v))
        .sum();
    raw * scale * scale
}

/// Calibration score (normalised digit mass on Q, W and E) of every candidate geometry.
/// `scales[i]` is the HUD scale that candidate `i` assumes.
#[must_use]
pub fn calibration_scores(candidates: &[Vec<HudRow>], scales: &[f64]) -> Vec<f64> {
    candidates
        .iter()
        .zip(scales)
        .map(|(rows, &scale)| basic_ability_mass(rows, scale))
        .collect()
}

/// Candidates scoring at least this fraction of the best belong to the same plateau:
/// neighbouring scales still see most of the digits, so the true scale lies within it.
const PLATEAU_RATIO: f64 = 0.7;

/// Picks, among candidate geometries, the one whose icons show cooldown digits.
///
/// `candidates[i]` is the per-second digit ratio of the icons for geometry `i`.
/// A wrong geometry reads background pixels and sees little of the digits, so the right
/// one stands out. Neighbouring scales may score almost as high, so the answer is the
/// centre of the plateau of high scores, and the margin is measured against candidates
/// *outside* that neighbourhood. Returns `None` when no geometry stands out (HUD hidden,
/// unusual layout, recording too short, or two distant candidates that both look
/// plausible).
#[must_use]
pub fn calibrate(candidates: &[Vec<HudRow>], scales: &[f64]) -> Option<usize> {
    let scores = calibration_scores(candidates, scales);
    let (best, &best_score) =
        scores
            .iter()
            .enumerate()
            .reduce(|top, cur| if *cur.1 > *top.1 { cur } else { top })?;
    if best_score <= 0.0 || basic_ability_episodes(&candidates[best]) < MIN_CALIBRATION_EPISODES {
        return None;
    }
    let on_plateau = |s: f64| s >= PLATEAU_RATIO * best_score;
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
        .fold(0.0, f64::max);
    if best_score < MIN_CALIBRATION_MARGIN * runner_up {
        return None;
    }
    let (weighted, total) = (lo..=hi).fold((0.0, 0.0), |(w, t), i| {
        (w + i as f64 * scores[i], t + scores[i])
    });
    Some((weighted / total).round() as usize)
}

/// Result of analysing the HUD of a recording.
#[derive(Debug, Clone, PartialEq)]
pub struct UltimateDetection {
    /// Index of the geometry (candidate) that was recognised.
    pub candidate: usize,
    /// Estimated ultimate cast times, in video seconds.
    pub casts_video_secs: Vec<f64>,
    /// Estimated cast times of each summoner spell (spell one, spell two), in video
    /// seconds.
    pub summoner_casts_video_secs: [Vec<f64>; 2],
}

/// Recognises the HUD geometry, then finds the ultimate and summoner-spell casts.
/// `None` when the HUD could not be recognised, so that no clip is produced from noise.
/// Recognises the HUD geometry, then finds the ultimate and summoner-spell casts.
/// `game_start_secs` is the second of the video at which the game starts (the recording
/// begins on the loading screen); it only matters for summoner spells.
/// `None` when the HUD could not be recognised, so that no clip is produced from noise.
#[must_use]
pub fn detect_ultimates(
    candidates: &[Vec<HudRow>],
    scales: &[f64],
    game_start_secs: usize,
) -> Option<UltimateDetection> {
    let candidate = calibrate(candidates, scales)?;
    let rows = &candidates[candidate];
    let column = |c: usize| -> Vec<f32> { rows.iter().map(|r| r[c]).collect() };
    Some(UltimateDetection {
        candidate,
        casts_video_secs: detect_casts(&column(ULTIMATE_ICON)),
        summoner_casts_video_secs: SUMMONER_BRIGHTNESS
            .map(|c| detect_summoner_casts(&column(c), game_start_secs)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Most tests do not care about the scales: these shadows give every candidate scale 1.
    fn unit_scales(candidates: &[Vec<HudRow>]) -> Vec<f64> {
        vec![1.0; candidates.len()]
    }

    fn calibrate(candidates: &[Vec<HudRow>]) -> Option<usize> {
        super::calibrate(candidates, &unit_scales(candidates))
    }

    fn calibration_scores(candidates: &[Vec<HudRow>]) -> Vec<f64> {
        super::calibration_scores(candidates, &unit_scales(candidates))
    }

    fn detect_ultimates(candidates: &[Vec<HudRow>]) -> Option<UltimateDetection> {
        super::detect_ultimates(candidates, &unit_scales(candidates), 0)
    }

    /// Rows where each listed icon shows digits during the given `(start, len)` spans.
    fn rows(len: usize, spans: &[(usize, usize, usize)]) -> Vec<HudRow> {
        let mut out = vec![[0.0f32; HUD_COLUMNS]; len];
        for &(icon, start, n) in spans {
            for row in out.iter_mut().skip(start).take(n) {
                row[icon] = 0.08;
            }
        }
        out
    }

    /// A geometry showing `per_icon` cooldown episodes on each of Q, W and E.
    fn plausible(per_icon: usize) -> Vec<HudRow> {
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
    fn digit_mass_picks_the_true_geometry_over_one_that_only_catches_digit_edges() {
        // A neighbouring geometry sees the edge of large digits: nearly as many episodes
        // (30 against 40), but much fainter and shorter.
        let mut alias = rows(600, &[]);
        for k in 0..30 {
            for row in alias.iter_mut().skip(10 + k * 18).take(4) {
                row[0] = 0.04;
            }
        }
        let mut spans = Vec::new();
        for k in 0..20 {
            spans.push((0, 10 + k * 25, 6));
            spans.push((2, 14 + k * 25, 6));
        }
        let strong = rows(600, &spans);
        assert_eq!(calibrate(&[alias, plausible(0), strong]), Some(2));
    }

    #[test]
    fn magnification_by_a_small_scale_does_not_make_it_win() {
        // The same digits, seen 1 / scale² stronger by the 0.8 candidate than by the 1.0 one.
        let digits = |ratio: f32| {
            let mut r = plausible(6);
            for row in &mut r {
                for v in row.iter_mut().filter(|v| **v > 0.0) {
                    *v = ratio;
                }
            }
            r
        };
        let candidates = [digits(0.125), digits(0.08)];
        let scores = super::calibration_scores(&candidates, &[0.8, 1.0]);
        assert!((scores[0] - scores[1]).abs() < 1e-6, "{scores:?}");
        // and the raw mass alone would have preferred the small scale by 56 %
        assert!(super::calibration_scores(&candidates, &[1.0, 1.0])[0] > 1.5 * scores[1]);
    }

    #[test]
    fn the_scores_are_the_digit_mass_of_q_w_and_e_only() {
        let mut r = rows(100, &[(0, 10, 10), (3, 10, 10), (4, 10, 10)]);
        r[50][1] = 0.02; // below the visibility threshold
        let scores = calibration_scores(&[r]);
        assert!((scores[0] - 0.8).abs() < 1e-6, "{scores:?}");
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
    fn summoner_casts_are_read_for_each_slot_from_the_recognised_geometry() {
        let mut right = plausible(6);
        // slot one dark from 120 s to 390 s, slot two from 40 s to 190 s
        for row in &mut right {
            row[SUMMONER_BRIGHTNESS[0]] = 0.54;
            row[SUMMONER_BRIGHTNESS[1]] = 0.54;
        }
        for row in right.iter_mut().skip(120).take(270) {
            row[SUMMONER_BRIGHTNESS[0]] = 0.42;
        }
        for row in right.iter_mut().skip(40).take(150) {
            row[SUMMONER_BRIGHTNESS[1]] = 0.42;
        }
        let d = detect_ultimates(&[plausible(0), right]).unwrap();
        assert_eq!(d.summoner_casts_video_secs[0], [119.5]);
        assert_eq!(d.summoner_casts_video_secs[1], [39.5]);
    }

    #[test]
    fn summoner_icons_do_not_count_for_the_calibration() {
        let mut only_summoners = plausible(0);
        for k in 0..10 {
            for row in only_summoners.iter_mut().skip(10 + k * 50).take(20) {
                row[4] = 0.08;
                row[SUMMONER_BRIGHTNESS[0]] = 0.4;
            }
        }
        assert_eq!(calibrate(&[only_summoners]), None);
    }

    /// Brightness of a summoner icon: `ready` normally, `dark` during each `(start, len)`.
    fn icon(len: usize, dark_spans: &[(usize, usize)]) -> Vec<f32> {
        let mut out = vec![0.54f32; len];
        for &(start, n) in dark_spans {
            for v in out.iter_mut().skip(start).take(n) {
                *v = 0.42;
            }
        }
        out
    }

    #[test]
    fn a_long_darkening_of_the_icon_is_a_cast_at_its_start() {
        let casts = detect_summoner_casts(&icon(900, &[(200, 270), (600, 250)]), 0);
        assert_eq!(casts, [199.5, 599.5]);
    }

    #[test]
    fn an_icon_that_never_darkens_has_no_cast() {
        assert!(detect_summoner_casts(&icon(900, &[]), 0).is_empty());
        let almost: Vec<f32> = (0..900).map(|i| 0.54 - (i % 2) as f32 * 0.005).collect();
        assert!(detect_summoner_casts(&almost, 0).is_empty());
    }

    #[test]
    fn a_short_darkening_is_not_a_summoner_cooldown() {
        // a death greys the HUD for a minute; a screen effect for a few seconds
        let s = icon(900, &[(100, 60), (300, 5), (500, 89)]);
        assert!(detect_summoner_casts(&s, 0).is_empty());
        assert_eq!(detect_summoner_casts(&icon(900, &[(100, 90)]), 0), [99.5]);
    }

    #[test]
    fn a_brief_bright_flash_inside_a_cooldown_does_not_split_it() {
        let mut s = icon(900, &[(200, 270)]);
        s[300] = 0.54;
        s[301] = 0.54;
        assert_eq!(detect_summoner_casts(&s, 0), [199.5]);
    }

    #[test]
    fn a_cooldown_already_running_when_the_game_starts_is_not_a_cast() {
        assert!(detect_summoner_casts(&icon(900, &[(0, 200)]), 0).is_empty());
        // the loading screen counts as nothing: the game starts at 70 s
        let mut s = icon(900, &[(70, 150), (400, 250)]);
        for v in s.iter_mut().take(70) {
            *v = 0.9; // a loading screen brighter than the HUD
        }
        assert_eq!(detect_summoner_casts(&s, 70), [399.5]);
    }

    #[test]
    fn a_single_short_cooldown_in_a_long_game_is_still_seen() {
        // 120 s of darkness in 3 600 s is 3 % of the game, well under a tenth
        assert_eq!(
            detect_summoner_casts(&icon(3600, &[(1500, 120)]), 0),
            [1499.5]
        );
    }

    #[test]
    fn a_loading_screen_brighter_than_the_hud_does_not_hide_the_ready_level() {
        let mut s = icon(600, &[(300, 250)]);
        for v in s.iter_mut().take(60) {
            *v = 0.9;
        }
        // without `from`, 10 % of the video is brighter than the icon and sets the level
        assert_eq!(detect_summoner_casts(&s, 60), [299.5]);
    }

    #[test]
    fn a_cooldown_cut_short_by_the_end_of_the_recording_is_still_a_cast_when_long_enough() {
        assert_eq!(detect_summoner_casts(&icon(600, &[(400, 200)]), 0), [399.5]);
        assert!(detect_summoner_casts(&icon(600, &[(560, 40)]), 0).is_empty());
    }

    #[test]
    fn too_short_a_recording_has_no_summoner_cast() {
        assert!(detect_summoner_casts(&icon(50, &[(10, 40)]), 0).is_empty());
        assert!(detect_summoner_casts(&[], 0).is_empty());
        assert!(detect_summoner_casts(&icon(100, &[]), 500).is_empty());
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
