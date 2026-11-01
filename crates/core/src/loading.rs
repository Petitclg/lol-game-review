//! Locates the end of the loading screen at the start of a recording.
//!
//! The Live Client API already answers during the loading screen, with a game clock
//! stuck at zero, so a recording usually starts well before game time 0. The loading
//! screen is almost static (champion portraits and a progress bar) and ends with a
//! hard cut to the game: the difference between two consecutive keyframes stays tiny,
//! then jumps.

/// Mean absolute difference (0-255) under which two frames are the same screen.
const STATIC_DIFF: f64 = 3.0;
/// Mean absolute difference over which two frames are different screens.
const CUT_DIFF: f64 = 10.0;
/// A loading screen shorter than this (in keyframe seconds) is not trusted.
const MIN_STATIC_SECS: usize = 3;
/// Game time (secs) shown by the first in-game frame, which lies somewhere in 0..1 s.
const FIRST_FRAME_GAME_SECS: f64 = 0.5;

/// Mean absolute difference between each frame and the next one; `raw` holds
/// consecutive gray frames of `frame_len` bytes.
#[must_use]
pub fn frame_diffs(raw: &[u8], frame_len: usize) -> Vec<f64> {
    if frame_len == 0 {
        return Vec::new();
    }
    let frames: Vec<&[u8]> = raw.chunks_exact(frame_len).collect();
    frames
        .windows(2)
        .map(|pair| {
            let sum: u64 = pair[0]
                .iter()
                .zip(pair[1])
                .map(|(a, b)| u64::from(a.abs_diff(*b)))
                .sum();
            sum as f64 / frame_len as f64
        })
        .collect()
}

/// Video time (secs) of the first in-game frame, given the differences between
/// consecutive one-per-second keyframes. `None` when the recording does not start on a
/// loading screen (it began in game, or the cut is not clear enough to be trusted).
#[must_use]
pub fn loading_end_secs(diffs: &[f64]) -> Option<f64> {
    let still = diffs.iter().take_while(|d| **d < STATIC_DIFF).count();
    let cut = *diffs.get(still)?;
    (still >= MIN_STATIC_SECS && cut >= CUT_DIFF).then_some((still + 1) as f64)
}

/// Game time at video time 0 for a recording whose game starts at `loading_end_secs`.
#[must_use]
pub fn anchor_from_loading_end(loading_end_secs: f64) -> f64 {
    FIRST_FRAME_GAME_SECS - loading_end_secs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diffs_are_mean_absolute_differences() {
        let raw = [0u8, 0, 10, 20, 10, 20];
        assert_eq!(frame_diffs(&raw, 2), [15.0, 0.0]);
    }

    #[test]
    fn diffs_ignore_a_trailing_partial_frame() {
        assert_eq!(frame_diffs(&[0, 0, 4, 4, 9], 2), [4.0]);
        assert!(frame_diffs(&[1, 2, 3], 2).is_empty());
        assert!(frame_diffs(&[1, 2, 3], 0).is_empty());
    }

    #[test]
    fn a_static_screen_then_a_cut_ends_the_loading() {
        let diffs = [0.2, 1.9, 0.1, 0.0, 0.3, 38.6, 14.5, 22.6];
        assert_eq!(loading_end_secs(&diffs), Some(6.0));
    }

    #[test]
    fn a_recording_starting_in_game_has_no_loading_screen() {
        assert_eq!(loading_end_secs(&[12.5, 5.9, 29.2, 18.8]), None);
        assert_eq!(loading_end_secs(&[0.1, 28.4, 27.7]), None);
    }

    #[test]
    fn a_soft_change_is_not_a_cut() {
        assert_eq!(loading_end_secs(&[0.1, 0.2, 0.1, 0.2, 5.0, 0.1]), None);
    }

    #[test]
    fn a_recording_that_never_changes_has_no_cut() {
        assert_eq!(loading_end_secs(&[0.0; 40]), None);
        assert_eq!(loading_end_secs(&[]), None);
    }

    #[test]
    fn the_anchor_precedes_the_cut() {
        assert!((anchor_from_loading_end(70.0) - -69.5).abs() < 1e-9);
    }
}
