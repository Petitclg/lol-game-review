//! Clip windows: padding around highlights and merging of overlapping ones.

use crate::events::Highlight;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ClipConfig {
    pub pre_roll_secs: f64,
    pub post_roll_secs: f64,
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            pre_roll_secs: 8.0,
            post_roll_secs: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipWindow {
    pub start_secs: f64,
    pub end_secs: f64,
    pub highlights: Vec<Highlight>,
}

/// Builds clip windows from time-sorted highlights; overlapping windows are merged
/// so a multikill produces a single clip. Start is clamped to 0.
#[must_use]
pub fn build_windows(highlights: &[Highlight], cfg: ClipConfig) -> Vec<ClipWindow> {
    let mut windows: Vec<ClipWindow> = Vec::new();
    for h in highlights {
        let start = (h.time_secs - cfg.pre_roll_secs).max(0.0);
        let end = h.time_secs + cfg.post_roll_secs;
        match windows.last_mut() {
            Some(last) if start <= last.end_secs => {
                last.end_secs = last.end_secs.max(end);
                last.highlights.push(h.clone());
            }
            _ => windows.push(ClipWindow {
                start_secs: start,
                end_secs: end,
                highlights: vec![h.clone()],
            }),
        }
    }
    windows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::Theme;

    fn hl(id: u32, t: f64) -> Highlight {
        Highlight {
            event_id: id,
            time_secs: t,
            theme: Theme::Kill,
            label: "Kill".into(),
            killer: None,
            victim: None,
            assisters: Vec::new(),
            caster: None,
        }
    }

    #[test]
    fn empty_input_gives_no_window() {
        assert_eq!(
            build_windows(&[], ClipConfig::default()),
            Vec::<ClipWindow>::new()
        );
    }

    #[test]
    fn pads_a_single_highlight() {
        let w = build_windows(&[hl(1, 100.0)], ClipConfig::default());
        assert_eq!(w.len(), 1);
        assert!((w[0].start_secs - 92.0).abs() < f64::EPSILON);
        assert!((w[0].end_secs - 108.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clamps_start_at_zero() {
        let w = build_windows(&[hl(1, 3.0)], ClipConfig::default());
        assert!(w[0].start_secs.abs() < f64::EPSILON);
    }

    #[test]
    fn merges_overlapping_windows() {
        let w = build_windows(&[hl(1, 100.0), hl(2, 108.0)], ClipConfig::default());
        assert_eq!(w.len(), 1);
        assert_eq!(w[0].highlights.len(), 2);
        assert!((w[0].end_secs - 116.0).abs() < f64::EPSILON);
    }

    #[test]
    fn keeps_distant_windows_separate() {
        let w = build_windows(&[hl(1, 100.0), hl(2, 200.0)], ClipConfig::default());
        assert_eq!(w.len(), 2);
    }
}
