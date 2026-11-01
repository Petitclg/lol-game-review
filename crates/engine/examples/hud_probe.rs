//! Diagnostic: recognises the HUD in a recording and lists the ultimate and summoner
//! spell casts (video seconds).
//!
//! `cargo run -p lgr-engine --example hud_probe -- <recording.mkv>` (read-only).

use lgr_core::loading::{frame_diffs, loading_end_secs};
use lgr_core::ultimate::{calibration_scores, detect_ultimates, SUMMONER_BRIGHTNESS};
use lgr_media::{
    extract_digit_ratios, extract_start_frames, locate_ffmpeg, SCALES, START_FRAME_LEN,
    START_SCAN_SECS,
};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let recording: PathBuf = std::env::args_os()
        .nth(1)
        .ok_or("usage: hud_probe <recording.mkv>")?
        .into();
    let ffmpeg = locate_ffmpeg(&[]).ok_or("ffmpeg not found")?;
    let rows = extract_digit_ratios(&ffmpeg, &recording, &SCALES)?;
    let start = extract_start_frames(&ffmpeg, &recording, START_SCAN_SECS)?;
    let game_start = loading_end_secs(&frame_diffs(&start, START_FRAME_LEN)).unwrap_or(0.0);
    println!(
        "seconds analysed: {} | game starts at {game_start} s",
        rows.first().map_or(0, Vec::len)
    );
    println!("normalised digit mass per candidate scale:");
    for (scale, score) in SCALES.iter().zip(calibration_scores(&rows, &SCALES)) {
        println!("  {scale:.1}: {score:.1}");
    }
    match detect_ultimates(&rows, &SCALES, game_start as usize) {
        Some(d) => {
            println!(
                "recognised scale {} | ultimate casts (video s): {:?}",
                SCALES[d.candidate], d.casts_video_secs
            );
            for (slot, casts) in d.summoner_casts_video_secs.iter().enumerate() {
                println!("  summoner spell {} casts (video s): {casts:?}", slot + 1);
            }
            for (slot, column) in SUMMONER_BRIGHTNESS.into_iter().enumerate() {
                let series: Vec<f32> = rows[d.candidate].iter().map(|r| r[column]).collect();
                let min = series.iter().copied().fold(f32::MAX, f32::min);
                let max = series.iter().copied().fold(0.0f32, f32::max);
                println!(
                    "  spell {} brightness: min {min:.3}, max {max:.3}",
                    slot + 1
                );
            }
        }
        None => println!("HUD NOT recognised"),
    }
    Ok(())
}
