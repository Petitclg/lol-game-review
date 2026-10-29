//! Reads the ability icons of the in-game HUD from a recording.
//!
//! Only keyframes are decoded (the recorder emits one per second) and only the row of
//! the four ability icons is kept, so analysing a whole game takes a few seconds of
//! low-priority CPU and never touches the game.
//!
//! Geometry was measured on a 2560x1440 recording with the default HUD and is
//! expressed in units of the frame height, so it scales with the resolution. The row is
//! centred horizontally. A custom HUD scale or a different layout will find no digits.

use crate::ffmpeg::silent_command;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::Path;
use std::process::Stdio;

/// Distance between the centres of two neighbouring ability icons.
const PITCH: f64 = 0.04076;
/// Height of an icon.
const ICON_H: f64 = 0.0358;
/// Vertical centre of the icon row.
const CENTER_Y: f64 = 0.9368;

/// Each icon cell of the extracted image is `CELL` x `CELL` pixels.
const CELL: usize = 32;
const FRAME_W: usize = CELL * 4;
const FRAME_BYTES: usize = FRAME_W * CELL;
/// Index of the ultimate (R) cell, the fourth icon.
pub const ULTIMATE_CELL: usize = 3;

/// Luma at or above which a pixel counts as part of a white cooldown digit.
const WHITE: u8 = 225;
/// Centre band of a cell where the cooldown digits are drawn; it excludes the mana cost
/// printed in the top-right corner, which is shown even when the ability is ready.
const BAND_ROWS: std::ops::Range<usize> = 12..23;
const BAND_COLS: std::ops::Range<usize> = 6..26;

fn video_filter() -> String {
    format!(
        "crop=w=4*{PITCH}*in_h:h={ICON_H}*in_h:x=in_w/2-3.5*{PITCH}*in_h:y={CENTER_Y}*in_h-{half}*in_h,\
         scale={FRAME_W}:{CELL}:flags=area,format=gray,fps=1",
        half = ICON_H / 2.0
    )
}

/// ffmpeg arguments writing the ability row of each keyframe second as raw gray frames.
#[must_use]
pub fn row_args(recording: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-threads", "2"]
        .map(OsString::from)
        .to_vec();
    a.extend(["-skip_frame", "nokey", "-i"].map(OsString::from));
    a.push(recording.into());
    a.extend(["-vf".into(), video_filter().into()]);
    a.extend(["-f", "rawvideo", "-"].map(OsString::from));
    a
}

/// Fraction of white pixels in the digit band of one icon cell.
#[must_use]
pub fn digit_ratio(frame: &[u8], cell: usize) -> f32 {
    let mut white = 0usize;
    let mut total = 0usize;
    for y in BAND_ROWS {
        for x in BAND_COLS {
            total += 1;
            if frame[y * FRAME_W + cell * CELL + x] >= WHITE {
                white += 1;
            }
        }
    }
    white as f32 / total as f32
}

/// For each second of the recording, the digit ratio of the four ability icons (Q, W, E, R).
pub fn extract_digit_ratios(ffmpeg: &Path, recording: &Path) -> io::Result<Vec<[f32; 4]>> {
    let mut cmd = silent_command(ffmpeg);
    cmd.args(row_args(recording))
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no ffmpeg output"))?;

    let mut frame = vec![0u8; FRAME_BYTES];
    let mut out = Vec::new();
    while read_frame(&mut stdout, &mut frame)? {
        out.push(std::array::from_fn(|cell| digit_ratio(&frame, cell)));
    }
    if child.wait()?.success() {
        Ok(out)
    } else {
        Err(io::Error::other("ffmpeg failed to read the recording"))
    }
}

/// Reads exactly one frame; `false` at a clean end of stream.
fn read_frame(reader: &mut impl Read, frame: &mut [u8]) -> io::Result<bool> {
    let mut filled = 0;
    while filled < frame.len() {
        let n = reader.read(&mut frame[filled..])?;
        if n == 0 {
            return Ok(false);
        }
        filled += n;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_with_digits_in(cell: usize) -> Vec<u8> {
        let mut f = vec![20u8; FRAME_BYTES];
        for y in BAND_ROWS {
            for x in 8..24 {
                f[y * FRAME_W + cell * CELL + x] = 255;
            }
        }
        f
    }

    #[test]
    fn digits_are_measured_only_in_their_own_cell() {
        let f = frame_with_digits_in(ULTIMATE_CELL);
        assert!(digit_ratio(&f, ULTIMATE_CELL) > 0.5);
        for other in 0..3 {
            assert!(digit_ratio(&f, other).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn a_ready_icon_has_no_digits() {
        let f = vec![90u8; FRAME_BYTES];
        assert!(digit_ratio(&f, ULTIMATE_CELL).abs() < f32::EPSILON);
    }

    #[test]
    fn the_mana_cost_in_the_corner_is_ignored() {
        let mut f = vec![20u8; FRAME_BYTES];
        for y in 0..8 {
            for x in 20..32 {
                f[y * FRAME_W + ULTIMATE_CELL * CELL + x] = 255;
            }
        }
        assert!(digit_ratio(&f, ULTIMATE_CELL).abs() < f32::EPSILON);
    }

    #[test]
    fn arguments_decode_keyframes_only_before_the_input() {
        let a: Vec<String> = row_args(Path::new("rec.mkv"))
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let skip = a.iter().position(|s| s == "-skip_frame").unwrap();
        let input = a.iter().position(|s| s == "-i").unwrap();
        assert!(skip < input);
        assert_eq!(a[skip + 1], "nokey");
        let vf = &a[a.iter().position(|s| s == "-vf").unwrap() + 1];
        assert!(vf.contains("crop=") && vf.contains("fps=1") && vf.contains("format=gray"));
        assert!(
            vf.contains("in_h"),
            "geometry must scale with the frame height"
        );
        assert_eq!(a.last().unwrap(), "-");
    }

    #[test]
    fn frames_are_read_whole_and_a_truncated_one_ends_the_stream() {
        let data = vec![1u8; FRAME_BYTES + 10];
        let mut reader = io::Cursor::new(data);
        let mut frame = vec![0u8; FRAME_BYTES];
        assert!(read_frame(&mut reader, &mut frame).unwrap());
        assert!(!read_frame(&mut reader, &mut frame).unwrap());
    }
}
