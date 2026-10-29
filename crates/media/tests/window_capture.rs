//! Real window capture, exercised on an animated ffplay window instead of the game.
//!
//! Needs an NVIDIA GPU (NVENC) and a full ffmpeg build with `gfxcapture` and `ffplay`;
//! skipped otherwise, so it only runs on a developer machine, never in CI.

use lgr_media::{detect_encoder, locate_ffmpeg, CaptureSource, Encoder, Recording};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::Duration;

/// The tests change a process-wide environment variable, so they must not overlap.
static ENV_LOCK: Mutex<()> = Mutex::new(());

struct Window(Child);

impl Drop for Window {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn open_animated_window(ffplay: &Path, title: &str) -> Window {
    let child = Command::new(ffplay)
        .args([
            "-loglevel",
            "quiet",
            "-f",
            "lavfi",
            "-i",
            "testsrc2=s=1280x720:r=60",
        ])
        .args(["-window_title", title, "-x", "1280", "-y", "720"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::thread::sleep(Duration::from_secs(4));
    Window(child)
}

/// Keyframe timestamps (seconds) of a recording.
fn keyframe_times(ffmpeg: &Path, video: &Path) -> Vec<f64> {
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-skip_frame", "nokey", "-i"])
        .arg(video)
        .args(["-vf", "showinfo", "-f", "null", "-"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stderr)
        .lines()
        .filter_map(|l| l.split("pts_time:").nth(1))
        .filter_map(|rest| rest.split_whitespace().next()?.parse().ok())
        .collect()
}

fn gpu_setup() -> Option<(PathBuf, PathBuf)> {
    let ffmpeg = locate_ffmpeg(&[])?;
    let ffplay = ffmpeg.with_file_name("ffplay.exe");
    if !ffplay.is_file() || detect_encoder(&ffmpeg) != Encoder::Nvenc {
        eprintln!("needs ffplay and an NVENC GPU, skipping");
        return None;
    }
    Some((ffmpeg, ffplay))
}

#[test]
fn a_window_is_recorded_with_exactly_one_keyframe_per_second() {
    let Some((ffmpeg, ffplay)) = gpu_setup() else {
        return;
    };
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let _window = open_animated_window(&ffplay, "lgr window capture test");
    std::env::set_var("LGR_CAPTURE_EXE", "ffplay");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("recording.mkv");

    let rec = Recording::start(&ffmpeg, Encoder::Nvenc, &out).unwrap();
    assert_eq!(rec.source(), CaptureSource::GameWindow);
    std::thread::sleep(Duration::from_secs(7));
    rec.stop().unwrap();

    let keys = keyframe_times(&ffmpeg, &out);
    assert!(keys.len() >= 5, "keyframes: {keys:?}");
    for pair in keys.windows(2) {
        assert!(
            (pair[1] - pair[0] - 1.0).abs() < 0.02,
            "keyframes must be one second apart: {keys:?}"
        );
    }
}

#[test]
fn recording_falls_back_to_the_screen_when_the_window_is_missing() {
    let Some((ffmpeg, _)) = gpu_setup() else {
        return;
    };
    let _guard = ENV_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    std::env::set_var("LGR_CAPTURE_EXE", "zz_no_such_game");
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("recording.mkv");
    let rec = Recording::start(&ffmpeg, Encoder::Nvenc, &out).unwrap();
    assert_eq!(rec.source(), CaptureSource::Desktop);
    rec.stop().unwrap();
}
