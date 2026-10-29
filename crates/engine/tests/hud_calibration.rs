//! Calibration tests on synthetic HUD videos: no game, no real footage needed.
//!
//! Each video draws white "cooldown digit" blocks on the four ability icons of a gray
//! frame, at the position a given resolution and HUD scale would put them. The real
//! pipeline (ffmpeg keyframe extraction, candidate crops, calibration, cast detection)
//! must find the scale and the ultimate casts. Skipped when ffmpeg is missing, unless
//! `LGR_REQUIRE_FFMPEG` is set.

use lgr_core::ultimate::{calibration_scores, detect_ultimates};
use lgr_media::{extract_digit_ratios, locate_ffmpeg, SCALES};
use std::path::Path;
use std::process::Command;

const DURATION: u32 = 240;
/// Ultimate cooldown spans (start, end) in video seconds.
const ULTIMATE_SPANS: [(u32, u32); 2] = [(60, 110), (150, 200)];

/// Geometry of the icon row, matching the measured HUD at scale 1.0.
struct Hud {
    width: f64,
    height: f64,
    scale: f64,
}

impl Hud {
    fn pitch(&self) -> f64 {
        0.04076 * self.scale * self.height
    }

    fn icon_h(&self) -> f64 {
        0.0358 * self.scale * self.height
    }

    fn center_y(&self) -> f64 {
        self.height - 0.0632 * self.scale * self.height
    }

    /// Where the cooldown digits are drawn for icon `k`: two thin vertical strokes in
    /// the centre band of its cell, like the strokes of real digits (a big block would be
    /// visible from neighbouring geometries too, which real digits are not).
    /// The four cells start at `width / 2 - 3.5 * pitch` and are one pitch wide.
    fn digit_strokes(&self, k: usize) -> Vec<(i64, i64, i64, i64)> {
        let cell_left = self.width / 2.0 - 3.5 * self.pitch() + k as f64 * self.pitch();
        let cell_top = self.center_y() - self.icon_h() / 2.0;
        let unit = self.pitch() / 32.0;
        [12.0, 18.0]
            .into_iter()
            .map(|col| {
                (
                    (cell_left + col * unit).round() as i64,
                    (cell_top + 0.42 * self.icon_h()).round() as i64,
                    (2.5 * unit).round().max(1.0) as i64,
                    (0.25 * self.icon_h()).round() as i64,
                )
            })
            .collect()
    }
}

/// Spans during which icon `k` shows digits: Q, W and E cycle constantly, R follows
/// the ultimate spans.
fn spans(k: usize) -> Vec<(u32, u32)> {
    if k == 3 {
        return ULTIMATE_SPANS.to_vec();
    }
    (0..DURATION / 20)
        .map(|i| {
            let start = 5 + i * 20 + k as u32 * 3;
            (start, start + 12)
        })
        .collect()
}

fn make_video(ffmpeg: &Path, out: &Path, hud: &Hud, with_hud: bool) {
    let mut filters: Vec<String> = Vec::new();
    if with_hud {
        for k in 0..4 {
            for (x, y, w, h) in hud.digit_strokes(k) {
                for (a, b) in spans(k) {
                    filters.push(format!(
                        "drawbox=x={x}:y={y}:w={w}:h={h}:color=white:t=fill:enable='between(t,{a},{b})'"
                    ));
                }
            }
        }
    }
    let vf = if filters.is_empty() {
        "null".to_owned()
    } else {
        filters.join(",")
    };
    let status = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg(format!(
            "color=c=0x303030:s={}x{}:r=2:d={DURATION}",
            hud.width as u32, hud.height as u32
        ))
        .args(["-vf", &vf, "-c:v", "mpeg4", "-q:v", "2", "-g", "2"])
        .arg(out)
        .status()
        .unwrap();
    assert!(status.success(), "could not generate the synthetic video");
}

fn ffmpeg_or_skip() -> Option<std::path::PathBuf> {
    let found = locate_ffmpeg(&[]);
    assert!(
        found.is_some() || std::env::var_os("LGR_REQUIRE_FFMPEG").is_none(),
        "ffmpeg is required but was not found"
    );
    found
}

fn analyse(width: f64, height: f64, scale: f64) -> Option<(f64, Vec<f64>)> {
    let ffmpeg = ffmpeg_or_skip()?;
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("hud.mkv");
    let hud = Hud {
        width,
        height,
        scale,
    };
    make_video(&ffmpeg, &video, &hud, true);
    let rows = extract_digit_ratios(&ffmpeg, &video, &SCALES).unwrap();
    eprintln!(
        "{width}x{height} scale {scale}: scores per candidate {:?} (scales {SCALES:?})",
        calibration_scores(&rows)
    );
    let d = detect_ultimates(&rows)?;
    Some((SCALES[d.candidate], d.casts_video_secs))
}

fn assert_recognised(width: f64, height: f64, scale: f64) {
    if ffmpeg_or_skip().is_none() {
        eprintln!("ffmpeg not found, skipping");
        return;
    }
    let (found, casts) = analyse(width, height, scale)
        .unwrap_or_else(|| panic!("HUD not recognised at {width}x{height}, scale {scale}"));
    assert!(
        (found - scale).abs() < 1e-9,
        "expected scale {scale}, found {found}"
    );
    assert_eq!(casts.len(), ULTIMATE_SPANS.len(), "casts: {casts:?}");
    for (cast, (start, _)) in casts.iter().zip(ULTIMATE_SPANS) {
        assert!(
            (cast - (f64::from(start) - 0.5)).abs() < 1e-9,
            "cast at {cast}, expected {}",
            f64::from(start) - 0.5
        );
    }
}

#[test]
fn the_measured_geometry_is_recognised_at_1080p() {
    assert_recognised(1920.0, 1080.0, 1.0);
}

#[test]
fn a_larger_hud_scale_is_recognised_at_1440p() {
    assert_recognised(2560.0, 1440.0, 1.3);
}

#[test]
fn a_small_resolution_with_a_large_hud_is_recognised() {
    assert_recognised(1280.0, 720.0, 1.5);
}

#[test]
fn an_ultrawide_frame_is_recognised_because_the_hud_is_centred() {
    assert_recognised(3440.0, 1440.0, 1.0);
}

#[test]
fn a_video_without_a_hud_is_reported_as_unrecognised() {
    let Some(ffmpeg) = ffmpeg_or_skip() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("empty.mkv");
    let hud = Hud {
        width: 1920.0,
        height: 1080.0,
        scale: 1.0,
    };
    make_video(&ffmpeg, &video, &hud, false);
    let rows = extract_digit_ratios(&ffmpeg, &video, &SCALES).unwrap();
    assert!(detect_ultimates(&rows).is_none());
}
