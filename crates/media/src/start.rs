//! Reads the first minutes of a recording as tiny gray frames, to find where the
//! loading screen ends (see `lgr_core::loading`).
//!
//! Only keyframes are decoded (the recorder emits one per second), so this takes a
//! fraction of a second of low-priority CPU and never touches the game.

use crate::ffmpeg::silent_command;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::Path;
use std::process::Stdio;

const FRAME_W: usize = 32;
const FRAME_H: usize = 18;
/// Size in bytes of one extracted frame.
pub const START_FRAME_LEN: usize = FRAME_W * FRAME_H;
/// A loading screen longer than this is not looked for.
pub const START_SCAN_SECS: u32 = 600;

/// ffmpeg arguments writing one raw gray frame per keyframe of the first `secs` seconds.
#[must_use]
pub fn start_args(recording: &Path, secs: u32) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-threads", "2"]
        .map(OsString::from)
        .to_vec();
    a.extend(["-skip_frame", "nokey", "-t"].map(OsString::from));
    a.push(secs.to_string().into());
    a.push("-i".into());
    a.push(recording.into());
    // Without passthrough ffmpeg duplicates frames to reach a constant frame rate.
    a.extend(["-fps_mode", "passthrough", "-vf"].map(OsString::from));
    a.push(format!("scale={FRAME_W}:{FRAME_H},format=gray").into());
    a.extend(["-f", "rawvideo", "-"].map(OsString::from));
    a
}

/// Raw gray frames (`START_FRAME_LEN` bytes each) of the keyframes in the first `secs`
/// seconds of the recording.
pub fn extract_start_frames(ffmpeg: &Path, recording: &Path, secs: u32) -> io::Result<Vec<u8>> {
    let mut cmd = silent_command(ffmpeg);
    cmd.args(start_args(recording, secs))
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut raw = Vec::new();
    child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no ffmpeg output"))?
        .read_to_end(&mut raw)?;
    if child.wait()?.success() {
        Ok(raw)
    } else {
        Err(io::Error::other("ffmpeg failed to read the recording"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_keyframes_are_decoded_without_frame_duplication() {
        let a = start_args(Path::new("rec.mkv"), 600);
        let pos = |s: &str| a.iter().position(|x| x == s).unwrap();
        assert!(pos("-skip_frame") < pos("-i"));
        assert_eq!(a[pos("-skip_frame") + 1], "nokey");
        assert_eq!(a[pos("-t") + 1], "600");
        assert!(pos("-t") < pos("-i"));
        assert_eq!(a[pos("-fps_mode") + 1], "passthrough");
    }

    #[test]
    fn frames_are_32_by_18_gray() {
        let a = start_args(Path::new("rec.mkv"), 60);
        let vf = a.iter().position(|x| x == "-vf").unwrap();
        assert_eq!(a[vf + 1], "scale=32:18,format=gray");
        assert_eq!(START_FRAME_LEN, 576);
    }
}
