//! Reads the ability icons of the in-game HUD from a recording.
//!
//! Only keyframes are decoded (the recorder emits one per second) and only the row of
//! the four ability icons is kept, so analysing a whole game takes a few seconds of
//! low-priority CPU and never touches the game.
//!
//! The row is read for several candidate HUD scales at once (one ffmpeg pass); the
//! caller picks the scale whose icons actually show cooldown digits. The base geometry
//! was measured on a 2560x1440 recording at the smallest HUD scale and is expressed in
//! units of the frame height, so it scales with the resolution. The row is anchored at
//! the bottom centre of the frame, like the HUD itself.

use crate::ffmpeg::silent_command;
use std::ffi::OsString;
use std::io::{self, Read};
use std::path::Path;
use std::process::Stdio;

/// Distance between the centres of two neighbouring ability icons (scale 1.0).
const PITCH: f64 = 0.04076;
/// Height of an icon (scale 1.0).
const ICON_H: f64 = 0.0358;
/// Distance from the bottom of the frame to the vertical centre of the icon row (scale 1.0).
const CENTER_FROM_BOTTOM: f64 = 0.0632;

/// Candidate HUD scales relative to the measured geometry.
pub const SCALES: [f64; 11] = [0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6, 1.8, 2.0];

/// Each icon cell of the extracted image is `CELL` x `CELL` pixels.
const CELL: usize = 32;
const FRAME_W: usize = CELL * 4;
/// Index of the ultimate (R) cell, the fourth icon.
pub const ULTIMATE_CELL: usize = 3;

/// Luma at or above which a pixel counts as part of a white cooldown digit.
const WHITE: u8 = 225;
/// Centre band of a cell where the cooldown digits are drawn; it excludes the mana cost
/// printed in the top-right corner, which is shown even when the ability is ready.
const BAND_ROWS: std::ops::Range<usize> = 12..23;
const BAND_COLS: std::ops::Range<usize> = 6..26;

fn frame_bytes(candidates: usize) -> usize {
    FRAME_W * CELL * candidates
}

/// `filter_complex` producing one image per second made of the ability row of every
/// candidate scale, stacked vertically.
fn filter_complex(scales: &[f64]) -> String {
    let n = scales.len();
    let labels = |prefix: char| -> String {
        (0..n)
            .map(|i| format!("[{prefix}{i}]"))
            .collect::<Vec<_>>()
            .concat()
    };
    let split_outputs = labels('v');
    let crops: Vec<String> = scales
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let pitch = PITCH * s;
            let icon = ICON_H * s;
            format!(
                "[v{i}]crop=w=4*{pitch}*in_h:h={icon}*in_h:x=in_w/2-3.5*{pitch}*in_h:\
                 y=in_h-{from_bottom}*in_h-{half}*in_h,\
                 scale={FRAME_W}:{CELL}:flags=area,format=gray,fps=1[c{i}]",
                from_bottom = CENTER_FROM_BOTTOM * s,
                half = icon / 2.0
            )
        })
        .collect();
    let stacked_inputs = labels('c');
    format!(
        "[0:v]split={n}{split_outputs};{};{stacked_inputs}vstack=inputs={n}[o]",
        crops.join(";")
    )
}

/// ffmpeg arguments writing, for each keyframe second, the ability row of every
/// candidate scale as raw gray frames.
#[must_use]
pub fn row_args(recording: &Path, scales: &[f64]) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-hide_banner", "-loglevel", "error", "-threads", "2"]
        .map(OsString::from)
        .to_vec();
    a.extend(["-skip_frame", "nokey", "-i"].map(OsString::from));
    a.push(recording.into());
    a.extend(["-filter_complex".into(), filter_complex(scales).into()]);
    a.extend(["-map", "[o]", "-f", "rawvideo", "-"].map(OsString::from));
    a
}

/// Fraction of white pixels in the digit band of one icon cell of one candidate.
#[must_use]
pub fn digit_ratio(frame: &[u8], candidate: usize, cell: usize) -> f32 {
    let mut white = 0usize;
    let mut total = 0usize;
    for y in BAND_ROWS {
        for x in BAND_COLS {
            total += 1;
            if frame[(candidate * CELL + y) * FRAME_W + cell * CELL + x] >= WHITE {
                white += 1;
            }
        }
    }
    white as f32 / total as f32
}

/// For each candidate scale, then each second of the recording, the digit ratio of the
/// four ability icons (Q, W, E, R).
pub fn extract_digit_ratios(
    ffmpeg: &Path,
    recording: &Path,
    scales: &[f64],
) -> io::Result<Vec<Vec<[f32; 4]>>> {
    let mut cmd = silent_command(ffmpeg);
    cmd.args(row_args(recording, scales))
        .stdin(Stdio::null())
        .stdout(Stdio::piped());
    let mut child = cmd.spawn()?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| io::Error::other("no ffmpeg output"))?;

    let mut frame = vec![0u8; frame_bytes(scales.len())];
    let mut out: Vec<Vec<[f32; 4]>> = vec![Vec::new(); scales.len()];
    while read_frame(&mut stdout, &mut frame)? {
        for (candidate, series) in out.iter_mut().enumerate() {
            series.push(std::array::from_fn(|cell| {
                digit_ratio(&frame, candidate, cell)
            }));
        }
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

    fn blank(candidates: usize) -> Vec<u8> {
        vec![20u8; frame_bytes(candidates)]
    }

    fn put_digits(frame: &mut [u8], candidate: usize, cell: usize) {
        for y in BAND_ROWS {
            for x in 8..24 {
                frame[(candidate * CELL + y) * FRAME_W + cell * CELL + x] = 255;
            }
        }
    }

    #[test]
    fn digits_are_measured_only_in_their_own_cell_and_candidate() {
        let mut f = blank(3);
        put_digits(&mut f, 1, ULTIMATE_CELL);
        assert!(digit_ratio(&f, 1, ULTIMATE_CELL) > 0.5);
        assert!(digit_ratio(&f, 1, 0).abs() < f32::EPSILON);
        assert!(digit_ratio(&f, 0, ULTIMATE_CELL).abs() < f32::EPSILON);
        assert!(digit_ratio(&f, 2, ULTIMATE_CELL).abs() < f32::EPSILON);
    }

    #[test]
    fn a_ready_icon_has_no_digits() {
        let f = vec![90u8; frame_bytes(1)];
        assert!(digit_ratio(&f, 0, ULTIMATE_CELL).abs() < f32::EPSILON);
    }

    #[test]
    fn the_mana_cost_in_the_corner_is_ignored() {
        let mut f = blank(1);
        for y in 0..8 {
            for x in 20..32 {
                f[y * FRAME_W + ULTIMATE_CELL * CELL + x] = 255;
            }
        }
        assert!(digit_ratio(&f, 0, ULTIMATE_CELL).abs() < f32::EPSILON);
    }

    #[test]
    fn the_filter_stacks_one_crop_per_candidate_scaled_to_the_frame_height() {
        let fc = filter_complex(&[1.0, 1.5]);
        assert!(fc.starts_with("[0:v]split=2[v0][v1];"));
        assert!(fc.ends_with("[c0][c1]vstack=inputs=2[o]"));
        assert_eq!(fc.matches("crop=").count(), 2);
        assert!(
            fc.contains("in_h"),
            "geometry must scale with the frame height"
        );
        // the larger candidate crops a larger area
        assert!(fc.contains("w=4*0.04076*in_h") && fc.contains("w=4*0.06114*in_h"));
    }

    #[test]
    fn arguments_decode_keyframes_only_before_the_input() {
        let a: Vec<String> = row_args(Path::new("rec.mkv"), &SCALES)
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        let skip = a.iter().position(|s| s == "-skip_frame").unwrap();
        let input = a.iter().position(|s| s == "-i").unwrap();
        assert!(skip < input);
        assert_eq!(a[skip + 1], "nokey");
        let fc = a.iter().position(|s| s == "-filter_complex").unwrap();
        assert!(a[fc + 1].contains(&format!("vstack=inputs={}", SCALES.len())));
        assert_eq!(a.last().unwrap(), "-");
    }

    #[test]
    fn frames_are_read_whole_and_a_truncated_one_ends_the_stream() {
        let n = frame_bytes(2);
        let mut reader = io::Cursor::new(vec![1u8; n + 10]);
        let mut frame = vec![0u8; n];
        assert!(read_frame(&mut reader, &mut frame).unwrap());
        assert!(!read_frame(&mut reader, &mut frame).unwrap());
    }
}
