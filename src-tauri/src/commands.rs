//! IPC commands (see docs/specs/ipc.md). Blocking work runs off the main thread.

use crate::state::AppState;
use lgr_core::session::Clip;
use lgr_core::settings::Settings;
use lgr_core::store::StorageUsage;
use lgr_engine::{
    finalize_generation, generate_clips_with as run_generation, load_clips as load_session_clips,
    merge_games, ultimate_status, FfmpegCutter, GameEntry, HudUltimateDetector, Status,
    UltimateStatus,
};
use lgr_lcu::{discover_lockfile_candidates, LcuApi, LcuClient};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    riot_id: String,
    level: u32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    session_id: String,
    done: usize,
    total: usize,
}

const CLIENT_OFF: &str = "Le client League of Legends n'est pas lancé.";

fn connect() -> Result<LcuClient, String> {
    LcuClient::connect(&discover_lockfile_candidates()).map_err(|e| match e {
        lgr_lcu::LcuError::NotRunning => CLIENT_OFF.to_owned(),
        other => other.to_string(),
    })
}

async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn get_account() -> Result<Option<Account>, String> {
    blocking(|| {
        let s = connect()?.current_summoner().map_err(|e| e.to_string())?;
        Ok(s.map(|s| Account {
            riot_id: s.riot_id(),
            level: s.summoner_level,
        }))
    })
    .await
}

#[tauri::command]
pub async fn list_games(state: State<'_, AppState>, count: u32) -> Result<Vec<GameEntry>, String> {
    let store = state.store.clone();
    blocking(move || {
        let matches = connect()?
            .recent_matches(count.clamp(1, 50))
            .map_err(|e| e.to_string())?;
        let sessions = store.list().map_err(|e| e.to_string())?;
        Ok(merge_games(&matches, &sessions))
    })
    .await
}

#[tauri::command]
pub async fn generate_clips(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<Clip>, String> {
    let store = state.store.clone();
    let settings = state.settings.clone();
    let ffmpeg = state.ffmpeg.clone();
    blocking(move || {
        let ffmpeg = ffmpeg.ok_or("ffmpeg est introuvable : impossible de générer les clips.")?;
        let cutter = FfmpegCutter {
            ffmpeg: ffmpeg.clone(),
        };
        let detector = HudUltimateDetector { ffmpeg };
        let id = session_id.clone();
        let clips = run_generation(&store, &cutter, &detector, &session_id, |done, total| {
            let _ = app.emit(
                "clips-progress",
                Progress {
                    session_id: id.clone(),
                    done,
                    total,
                },
            );
        })
        .map_err(|e| e.to_string())?;
        // Reclaims the source video when the user asked for it; a failure here must not
        // fail a generation that succeeded.
        let _ = finalize_generation(&store, &settings.load(), &session_id, &clips);
        Ok(clips)
    })
    .await
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.load()
}

#[tauri::command]
pub fn set_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, String> {
    state.settings.save(settings).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_storage_usage(state: State<'_, AppState>) -> Result<StorageUsage, String> {
    let store = state.store.clone();
    blocking(move || store.usage().map_err(|e| e.to_string())).await
}

#[tauri::command]
pub async fn get_clips(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Vec<Clip>, String> {
    let store = state.store.clone();
    blocking(move || load_session_clips(&store, &session_id).map_err(|e| e.to_string())).await
}

#[tauri::command]
pub async fn get_ultimate_status(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<UltimateStatus, String> {
    let store = state.store.clone();
    blocking(move || ultimate_status(&store, &session_id).map_err(|e| e.to_string())).await
}

#[tauri::command]
pub async fn delete_recording(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let store = state.store.clone();
    blocking(move || {
        store
            .delete_recording(&session_id)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub fn get_status(state: State<'_, AppState>) -> Result<Status, String> {
    state
        .status
        .lock()
        .map(|s| s.clone())
        .map_err(|e| e.to_string())
}
