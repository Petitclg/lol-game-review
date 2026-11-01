//! IPC commands (see docs/specs/ipc.md). Blocking work runs off the main thread.

use crate::generation;
use crate::state::AppState;
use lgr_core::annotations::{Annotation, Annotations};
use lgr_core::session::Clip;
use lgr_core::settings::Settings;
use lgr_core::store::StorageUsage;
use lgr_engine::{
    list_annotated_moments as read_annotated_moments, load_clips as load_session_clips,
    merge_games, remember_games, ultimate_status, AnnotatedMoment, GameEntry, Status,
    UltimateStatus,
};
use lgr_lcu::{discover_lockfile_candidates, LcuApi, LcuClient};
use serde::Serialize;
use tauri::{AppHandle, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    riot_id: String,
    level: u32,
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
        let games = merge_games(&matches, &sessions);
        // champion, outcome and date are only known to the client, whose history is
        // limited: keep them with the game for the review of annotations
        remember_games(&store, &games);
        Ok(games)
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
    let in_flight = state.in_flight.clone();
    blocking(move || generation::generate(&app, &store, &settings, ffmpeg, &in_flight, &session_id))
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
pub async fn get_annotations(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<Annotations, String> {
    let store = state.store.clone();
    blocking(move || {
        store
            .load_annotations(&session_id)
            .map_err(|e| e.to_string())
    })
    .await
}

#[tauri::command]
pub async fn set_annotation(
    state: State<'_, AppState>,
    session_id: String,
    part_id: String,
    tags: Vec<String>,
) -> Result<Annotation, String> {
    let store = state.store.clone();
    blocking(move || {
        store
            .set_annotation(&session_id, &part_id, &tags)
            .map_err(|e| e.to_string())
    })
    .await
}

/// The custom tags offered on every highlight.
#[tauri::command]
pub fn get_custom_tags(state: State<'_, AppState>) -> Vec<String> {
    state.custom_tags.load()
}

/// The list after an addition, and the text to put on the highlight (the existing one when
/// the tag was already offered, whatever its case).
#[derive(Serialize)]
pub struct AddedCustomTag {
    labels: Vec<String>,
    label: String,
}

#[tauri::command]
pub fn add_custom_tag(state: State<'_, AppState>, label: String) -> Result<AddedCustomTag, String> {
    state
        .custom_tags
        .add(&label)
        .map(|(labels, label)| AddedCustomTag { labels, label })
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_custom_tag(state: State<'_, AppState>, label: String) -> Result<Vec<String>, String> {
    state.custom_tags.remove(&label).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_annotated_moments(
    state: State<'_, AppState>,
) -> Result<Vec<AnnotatedMoment>, String> {
    let store = state.store.clone();
    blocking(move || read_annotated_moments(&store).map_err(|e| e.to_string())).await
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
