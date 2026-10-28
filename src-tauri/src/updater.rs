//! Update to the latest release at startup, never while a game is running.

use crate::state::AppState;
use lgr_engine::GameState;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

pub fn check_on_startup(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = try_update(&app).await {
            eprintln!("update check failed: {e}");
        }
    });
}

async fn try_update(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };
    let in_game = app
        .state::<AppState>()
        .status
        .lock()
        .map_or(true, |s| s.state == GameState::InGame);
    if in_game {
        return Ok(());
    }
    update.download_and_install(|_, _| {}, || {}).await?;
    app.restart();
}
