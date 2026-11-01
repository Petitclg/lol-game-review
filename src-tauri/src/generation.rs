//! Clip generation of one game, shared by the `generate_clips` command and the automatic
//! generation at the end of a game.

use lgr_core::session::Clip;
use lgr_core::settings::SettingsStore;
use lgr_core::store::SessionStore;
use lgr_engine::{
    finalize_generation, generate_clips_with, FfmpegCutter, HudUltimateDetector, InFlight,
};
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    session_id: String,
    done: usize,
    total: usize,
}

/// Sent when an automatic generation ends, so that the window can show the new clips.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Generated {
    session_id: String,
    /// `None` when the clips were generated.
    error: Option<String>,
}

pub const ALREADY_RUNNING: &str = "La génération des clips de cette partie est déjà en cours.";

/// Cuts the clips of `session_id` (and reclaims its video if the user asked for it),
/// reporting progress to the window. A game that is already being generated is refused.
pub fn generate(
    app: &AppHandle,
    store: &SessionStore,
    settings: &SettingsStore,
    ffmpeg: Option<PathBuf>,
    in_flight: &InFlight,
    session_id: &str,
) -> Result<Vec<Clip>, String> {
    let _token = in_flight
        .acquire(session_id)
        .ok_or_else(|| ALREADY_RUNNING.to_owned())?;
    let ffmpeg = ffmpeg.ok_or("ffmpeg est introuvable : impossible de générer les clips.")?;
    let cutter = FfmpegCutter {
        ffmpeg: ffmpeg.clone(),
    };
    let detector = HudUltimateDetector { ffmpeg };
    let clips = generate_clips_with(store, &cutter, &detector, session_id, |done, total| {
        let _ = app.emit(
            "clips-progress",
            Progress {
                session_id: session_id.to_owned(),
                done,
                total,
            },
        );
    })
    .map_err(|e| e.to_string())?;
    // A failure to reclaim the video must not fail a generation that succeeded.
    let _ = finalize_generation(store, &settings.load(), session_id, &clips);
    Ok(clips)
}

/// Tells the window that an automatic generation ended.
pub fn announce(app: &AppHandle, session_id: &str, outcome: &Result<Vec<Clip>, String>) {
    let _ = app.emit(
        "clips-generated",
        Generated {
            session_id: session_id.to_owned(),
            error: outcome.as_ref().err().cloned(),
        },
    );
}
