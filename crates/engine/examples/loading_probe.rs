//! Diagnostic: finds where the loading screen ends in recordings (read-only).
//!
//! `cargo run -p lgr-engine --example loading_probe -- <recording.mkv>...`

use lgr_core::loading::{anchor_from_loading_end, frame_diffs, loading_end_secs};
use lgr_media::{extract_start_frames, locate_ffmpeg, START_FRAME_LEN, START_SCAN_SECS};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ffmpeg = locate_ffmpeg(&[]).ok_or("ffmpeg not found")?;
    for recording in std::env::args_os().skip(1).map(PathBuf::from) {
        let raw = extract_start_frames(&ffmpeg, &recording, START_SCAN_SECS)?;
        let diffs = frame_diffs(&raw, START_FRAME_LEN);
        match loading_end_secs(&diffs) {
            Some(end) => println!(
                "{}: loading ends at {end} s, anchor {:.1}",
                recording.display(),
                anchor_from_loading_end(end)
            ),
            None => println!("{}: no loading screen", recording.display()),
        }
    }
    Ok(())
}
