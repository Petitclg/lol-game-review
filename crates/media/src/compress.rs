//! Re-encoding of clips with a GPU encoder, in a higher-quality, lower-bitrate mode.
//!
//! Recordings are encoded in real time with a fast preset, which is light on the game
//! but produces large files. Clips are small, so they can be re-encoded afterwards
//! (while no game is running) with a slower preset for roughly half the size.

use crate::ffmpeg::{run_with_timeout, silent_command, Encoder};
use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;

fn codec_args(encoder: Encoder) -> Option<Vec<&'static str>> {
    Some(match encoder {
        Encoder::Nvenc => vec![
            "-c:v",
            "h264_nvenc",
            "-preset",
            "p6",
            "-tune",
            "hq",
            "-rc",
            "vbr",
            "-cq",
            "36",
            "-b:v",
            "0",
            "-rc-lookahead",
            "24",
            "-bf",
            "3",
            "-spatial-aq",
            "1",
        ],
        Encoder::Amf => vec![
            "-c:v", "h264_amf", "-quality", "quality", "-rc", "cqp", "-qp_i", "34", "-qp_p", "36",
            "-qp_b", "38",
        ],
        Encoder::Qsv => vec![
            "-c:v",
            "h264_qsv",
            "-preset",
            "slower",
            "-global_quality",
            "34",
        ],
        // CPU encoders are too heavy to run next to a game and LGPL builds lack x264.
        Encoder::X264 | Encoder::OpenH264 => return None,
    })
}

/// Arguments re-encoding `input` into `output` (video only, fast-start MP4).
#[must_use]
pub fn compress_args(encoder: Encoder, input: &Path, output: &Path) -> Option<Vec<OsString>> {
    let codec = codec_args(encoder)?;
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-y", "-i"]
        .map(OsString::from)
        .to_vec();
    a.push(input.into());
    a.extend(codec.into_iter().map(OsString::from));
    a.extend(["-pix_fmt", "yuv420p", "-an", "-movflags", "+faststart"].map(OsString::from));
    a.push(output.into());
    Some(a)
}

/// Arguments encoding a few synthetic frames to nowhere, to validate an encoder.
fn probe_args(encoder: Encoder) -> Option<Vec<OsString>> {
    let codec = codec_args(encoder)?;
    let mut a: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        "color=c=black:s=640x360:r=30",
        "-frames:v",
        "5",
    ]
    .map(OsString::from)
    .to_vec();
    a.extend(codec.into_iter().map(OsString::from));
    a.extend(["-pix_fmt", "yuv420p", "-f", "null", "-"].map(OsString::from));
    Some(a)
}

/// The first GPU encoder able to compress clips on this machine, if any.
#[must_use]
pub fn detect_compress_encoder(ffmpeg: &Path) -> Option<Encoder> {
    [Encoder::Nvenc, Encoder::Amf, Encoder::Qsv]
        .into_iter()
        .find(|&enc| {
            probe_args(enc).is_some_and(|args| {
                let mut cmd = silent_command(ffmpeg);
                cmd.args(args);
                run_with_timeout(cmd, Duration::from_secs(20)).unwrap_or(false)
            })
        })
}

/// Re-encodes `input` into `output`. The caller decides whether to keep the result.
pub fn compress_clip(
    ffmpeg: &Path,
    encoder: Encoder,
    input: &Path,
    output: &Path,
) -> io::Result<()> {
    let args = compress_args(encoder, input, output)
        .ok_or_else(|| io::Error::other("no GPU encoder available for compression"))?;
    let mut cmd = silent_command(ffmpeg);
    cmd.args(args);
    if run_with_timeout(cmd, Duration::from_secs(120))? && output.is_file() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(output);
        Err(io::Error::other("ffmpeg failed to compress the clip"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(v: &[OsString]) -> Vec<String> {
        v.iter().map(|s| s.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn gpu_encoders_get_a_quality_oriented_preset() {
        let a = strs(
            &compress_args(Encoder::Nvenc, Path::new("in.mp4"), Path::new("out.mp4")).unwrap(),
        );
        assert!(a.windows(2).any(|w| w == ["-c:v", "h264_nvenc"]));
        assert!(a.windows(2).any(|w| w == ["-preset", "p6"]));
        assert!(a.windows(2).any(|w| w == ["-rc", "vbr"]));
        assert!(a.contains(&"-an".to_owned()));
        assert!(a.windows(2).any(|w| w == ["-movflags", "+faststart"]));
        assert_eq!(a.last().unwrap(), "out.mp4");
        let input = a.iter().position(|s| s == "-i").unwrap();
        assert_eq!(a[input + 1], "in.mp4");
    }

    #[test]
    fn nvenc_uses_the_measured_quality_setting() {
        // Measured on a real clip: cq 36 halves the size (SSIM 0.967); cq 30 saves nothing.
        let a = strs(&compress_args(Encoder::Nvenc, Path::new("i"), Path::new("o")).unwrap());
        assert!(a.windows(2).any(|w| w == ["-cq", "36"]));
    }

    #[test]
    fn cpu_encoders_are_never_used_for_compression() {
        assert!(compress_args(Encoder::X264, Path::new("a"), Path::new("b")).is_none());
        assert!(compress_args(Encoder::OpenH264, Path::new("a"), Path::new("b")).is_none());
        assert!(probe_args(Encoder::X264).is_none());
    }

    #[test]
    fn the_probe_encodes_synthetic_frames_to_nowhere() {
        let a = strs(&probe_args(Encoder::Amf).unwrap());
        assert!(a.windows(2).any(|w| w == ["-frames:v", "5"]));
        assert_eq!(&a[a.len() - 3..], ["-f", "null", "-"]);
    }
}
