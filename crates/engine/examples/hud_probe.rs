//! Diagnostic: recognises the HUD in a recording and lists the ultimate casts.
//!
//! `cargo run -p lgr-engine --example hud_probe -- <recording.mkv>` (read-only).

use lgr_core::ultimate::{calibration_scores, detect_ultimates};
use lgr_media::{extract_digit_ratios, locate_ffmpeg, SCALES};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recording: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("usage: hud_probe <recording.mkv>")?
        .into();
    let ffmpeg = locate_ffmpeg(&[]).ok_or("ffmpeg not found")?;
    let rows = extract_digit_ratios(&ffmpeg, &recording, &SCALES)?;
    println!("seconds analysed: {}", rows.first().map_or(0, Vec::len));
    println!("scores per candidate scale:");
    for (scale, score) in SCALES.iter().zip(calibration_scores(&rows)) {
        println!("  {scale:.1}: {score}");
    }
    match detect_ultimates(&rows) {
        Some(d) => println!(
            "recognised scale {} | casts (video s): {:?}",
            SCALES[d.candidate], d.casts_video_secs
        ),
        None => println!("HUD NOT recognised"),
    }
    Ok(())
}
