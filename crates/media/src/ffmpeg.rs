//! ffmpeg discovery, command-line construction and silent process spawning.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    Nvenc,
    Amf,
    Qsv,
    /// CPU fallback; noticeably heavier, avoided whenever a GPU encoder works.
    X264,
}

const GPU_ENCODERS: [Encoder; 3] = [Encoder::Nvenc, Encoder::Amf, Encoder::Qsv];

impl Encoder {
    fn args(self) -> Vec<&'static str> {
        match self {
            Self::Nvenc => vec![
                "-c:v",
                "h264_nvenc",
                "-preset",
                "p1",
                "-tune",
                "ll",
                "-rc",
                "vbr",
                "-cq",
                "30",
                "-b:v",
                "4M",
                "-maxrate",
                "6M",
                "-g",
                "30",
            ],
            Self::Amf => vec![
                "-vf",
                "hwdownload,format=bgra,format=nv12",
                "-c:v",
                "h264_amf",
                "-usage",
                "lowlatency",
                "-quality",
                "speed",
                "-rc",
                "vbr_peak",
                "-b:v",
                "4M",
                "-maxrate",
                "6M",
                "-g",
                "30",
            ],
            Self::Qsv => vec![
                "-vf",
                "hwdownload,format=bgra,format=nv12",
                "-c:v",
                "h264_qsv",
                "-preset",
                "veryfast",
                "-b:v",
                "4M",
                "-maxrate",
                "6M",
                "-g",
                "30",
            ],
            Self::X264 => vec![
                "-vf",
                "hwdownload,format=bgra,scale=-2:720,format=yuv420p",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-crf",
                "30",
                "-g",
                "30",
            ],
        }
    }
}

const CAPTURE_INPUT: &str = "ddagrab=output_idx=0:framerate=30";
const BASE_ARGS: [&str; 4] = ["-hide_banner", "-loglevel", "error", "-y"];

/// Arguments recording the primary display, video only, one keyframe per second
/// (so clips can be cut without re-encoding).
#[must_use]
pub fn record_args(encoder: Encoder, output: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = BASE_ARGS.iter().map(OsString::from).collect();
    a.extend(["-f", "lavfi", "-i", CAPTURE_INPUT].map(OsString::from));
    a.extend(encoder.args().into_iter().map(OsString::from));
    a.push("-an".into());
    a.push(output.into());
    a
}

/// Arguments encoding a single captured frame to nowhere, to validate an encoder.
#[must_use]
pub fn probe_args(encoder: Encoder) -> Vec<OsString> {
    let mut a: Vec<OsString> = BASE_ARGS.iter().map(OsString::from).collect();
    a.extend(["-f", "lavfi", "-i", CAPTURE_INPUT].map(OsString::from));
    a.extend(encoder.args().into_iter().map(OsString::from));
    a.extend(["-frames:v", "1", "-f", "null", "-"].map(OsString::from));
    a
}

/// Arguments cutting `[start, end]` seconds without re-encoding.
#[must_use]
pub fn cut_args(input: &Path, start_secs: f64, end_secs: f64, output: &Path) -> Vec<OsString> {
    let duration = (end_secs - start_secs).max(0.1);
    let mut a: Vec<OsString> = BASE_ARGS.iter().map(OsString::from).collect();
    a.push("-ss".into());
    a.push(format!("{start_secs:.3}").into());
    a.push("-i".into());
    a.push(input.into());
    a.push("-t".into());
    a.push(format!("{duration:.3}").into());
    a.extend(
        [
            "-c",
            "copy",
            "-avoid_negative_ts",
            "make_zero",
            "-movflags",
            "+faststart",
        ]
        .map(OsString::from),
    );
    a.push(output.into());
    a
}

/// Candidate ffmpeg locations in priority order. Pure so it can be tested.
#[must_use]
pub fn ffmpeg_candidates(
    env_override: Option<PathBuf>,
    extra_dirs: &[PathBuf],
    path_var: Option<&OsString>,
    local_app_data: Option<&Path>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = env_override.into_iter().collect();
    out.extend(extra_dirs.iter().map(|d| d.join("ffmpeg.exe")));
    if let Some(p) = path_var {
        out.extend(std::env::split_paths(p).map(|d| d.join("ffmpeg.exe")));
    }
    if let Some(l) = local_app_data {
        out.push(
            l.join("Microsoft")
                .join("WinGet")
                .join("Links")
                .join("ffmpeg.exe"),
        );
    }
    out
}

/// `…\WinGet\Packages\Gyan.FFmpeg*\ffmpeg-*\bin\ffmpeg.exe`, as installed by winget.
fn winget_package_ffmpeg(local_app_data: &Path) -> Vec<PathBuf> {
    let packages = local_app_data
        .join("Microsoft")
        .join("WinGet")
        .join("Packages");
    let Ok(entries) = std::fs::read_dir(packages) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for pkg in entries.flatten() {
        if !pkg.file_name().to_string_lossy().starts_with("Gyan.FFmpeg") {
            continue;
        }
        if let Ok(builds) = std::fs::read_dir(pkg.path()) {
            out.extend(
                builds
                    .flatten()
                    .map(|b| b.path().join("bin").join("ffmpeg.exe")),
            );
        }
    }
    out
}

/// Finds ffmpeg: `LGR_FFMPEG`, bundled directories, `PATH`, then winget locations.
#[must_use]
pub fn locate_ffmpeg(extra_dirs: &[PathBuf]) -> Option<PathBuf> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let mut candidates = ffmpeg_candidates(
        std::env::var_os("LGR_FFMPEG").map(PathBuf::from),
        extra_dirs,
        std::env::var_os("PATH").as_ref(),
        local.as_deref(),
    );
    if let Some(l) = &local {
        candidates.extend(winget_package_ffmpeg(l));
    }
    candidates.into_iter().find(|p| p.is_file())
}

/// A command that never opens a window and runs below normal priority, so it cannot
/// steal focus or compete with the game.
#[must_use]
pub fn silent_command(program: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    cmd
}

/// Runs `cmd` and reports whether it exited successfully within `timeout`.
pub fn run_with_timeout(mut cmd: Command, timeout: Duration) -> io::Result<bool> {
    let mut child = cmd.stdin(Stdio::null()).spawn()?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status.success());
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Ok(false);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Picks the first GPU encoder that really works on this machine, else the CPU one.
#[must_use]
pub fn detect_encoder(ffmpeg: &Path) -> Encoder {
    for enc in GPU_ENCODERS {
        let mut cmd = silent_command(ffmpeg);
        cmd.args(probe_args(enc));
        if run_with_timeout(cmd, Duration::from_secs(15)).unwrap_or(false) {
            return enc;
        }
    }
    Encoder::X264
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(v: &[OsString]) -> Vec<String> {
        v.iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn record_args_are_silent_video_only_and_keyframed() {
        let a = strs(&record_args(Encoder::Nvenc, Path::new("out.mkv")));
        assert!(a.contains(&"-an".to_owned()));
        assert!(a.windows(2).any(|w| w == ["-g", "30"]));
        assert!(a.windows(2).any(|w| w == ["-c:v", "h264_nvenc"]));
        assert_eq!(a.last().unwrap(), "out.mkv");
        assert!(a.windows(2).any(|w| w == ["-loglevel", "error"]));
    }

    #[test]
    fn cpu_fallback_downscales() {
        let a = strs(&record_args(Encoder::X264, Path::new("o.mkv")));
        assert!(a.iter().any(|s| s.contains("scale=-2:720")));
        assert!(a.contains(&"libx264".to_owned()));
    }

    #[test]
    fn probe_encodes_one_frame_to_null() {
        let a = strs(&probe_args(Encoder::Amf));
        assert!(a.windows(2).any(|w| w == ["-frames:v", "1"]));
        assert_eq!(&a[a.len() - 3..], ["-f", "null", "-"]);
    }

    #[test]
    fn cut_args_copy_streams_with_seek_before_input() {
        let a = strs(&cut_args(
            Path::new("in.mkv"),
            10.0,
            22.5,
            Path::new("o.mp4"),
        ));
        let ss = a.iter().position(|s| s == "-ss").unwrap();
        let i = a.iter().position(|s| s == "-i").unwrap();
        assert!(ss < i);
        assert_eq!(a[ss + 1], "10.000");
        assert!(a.windows(2).any(|w| w == ["-t", "12.500"]));
        assert!(a.windows(2).any(|w| w == ["-c", "copy"]));
    }

    #[test]
    fn cut_args_enforce_minimum_duration() {
        let a = strs(&cut_args(Path::new("i"), 5.0, 5.0, Path::new("o")));
        assert!(a.windows(2).any(|w| w == ["-t", "0.100"]));
    }

    #[test]
    fn candidates_follow_priority_order() {
        let path = OsString::from(r"C:\bin;D:\tools");
        let c = ffmpeg_candidates(
            Some(PathBuf::from(r"X:\override\ffmpeg.exe")),
            &[PathBuf::from(r"C:\app")],
            Some(&path),
            Some(Path::new(r"C:\Users\u\AppData\Local")),
        );
        assert_eq!(c[0], PathBuf::from(r"X:\override\ffmpeg.exe"));
        assert_eq!(c[1], PathBuf::from(r"C:\app").join("ffmpeg.exe"));
        assert_eq!(c[2], PathBuf::from(r"C:\bin").join("ffmpeg.exe"));
        assert!(c.last().unwrap().ends_with("ffmpeg.exe"));
        assert!(c.last().unwrap().to_string_lossy().contains("WinGet"));
    }
}
