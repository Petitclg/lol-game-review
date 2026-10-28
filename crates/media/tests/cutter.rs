//! Runs only when ffmpeg is available; otherwise the tests are skipped.

use lgr_media::{cut_clip, locate_ffmpeg};
use std::path::Path;
use std::process::Command;

fn duration_secs(ffmpeg: &Path, file: &Path) -> f64 {
    let out = Command::new(ffmpeg).arg("-i").arg(file).output().unwrap();
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    let d = text
        .split("Duration: ")
        .nth(1)
        .and_then(|s| s.get(..11))
        .expect("duration line");
    let p: Vec<f64> = d.split(':').map(|x| x.parse().unwrap()).collect();
    p[0] * 3600.0 + p[1] * 60.0 + p[2]
}

/// Skips when ffmpeg is missing, unless `LGR_REQUIRE_FFMPEG` is set (CI).
fn require_ffmpeg() -> Option<std::path::PathBuf> {
    let found = locate_ffmpeg(&[]);
    assert!(
        found.is_some() || std::env::var_os("LGR_REQUIRE_FFMPEG").is_none(),
        "ffmpeg is required but was not found"
    );
    if found.is_none() {
        eprintln!("ffmpeg not found, skipping");
    }
    found
}

#[test]
fn cuts_the_requested_range_without_reencoding() {
    let Some(ffmpeg) = require_ffmpeg() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("in.mkv");
    let ok = Command::new(&ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=size=160x120:rate=30")
        .args([
            "-t", "8", "-c:v", "libx264", "-g", "30", "-pix_fmt", "yuv420p",
        ])
        .arg(&input)
        .status()
        .unwrap()
        .success();
    assert!(ok, "could not generate test video");

    let out = dir.path().join("clip.mp4");
    cut_clip(&ffmpeg, &input, 2.0, 5.0, &out).unwrap();
    let d = duration_secs(&ffmpeg, &out);
    assert!((2.5..=4.2).contains(&d), "duration was {d}");
}

#[test]
fn failed_cut_leaves_no_file() {
    let Some(ffmpeg) = require_ffmpeg() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("clip.mp4");
    let err = cut_clip(&ffmpeg, &dir.path().join("missing.mkv"), 0.0, 1.0, &out);
    assert!(err.is_err());
    assert!(!out.exists());
}
