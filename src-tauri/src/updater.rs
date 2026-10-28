//! Keeps the app on the latest release: checks at startup, retries on failure and
//! re-checks periodically. Installation never happens while a game is running.

use crate::state::AppState;
use lgr_engine::{next_check_delay, GameState, UpdateOutcome};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

const MAX_LOG_BYTES: u64 = 100 * 1024;

pub fn start(app: AppHandle, log_path: PathBuf) {
    std::thread::spawn(move || {
        log(
            &log_path,
            &format!(
                "updater started, current version {}",
                app.package_info().version
            ),
        );
        let mut failures = 0usize;
        loop {
            let outcome = match tauri::async_runtime::block_on(try_update(&app, &log_path)) {
                Ok(outcome) => outcome,
                Err(e) => {
                    log(&log_path, &format!("update failed: {e}"));
                    UpdateOutcome::Failed
                }
            };
            failures = if outcome == UpdateOutcome::Failed {
                failures + 1
            } else {
                0
            };
            let delay = next_check_delay(outcome, failures);
            log(
                &log_path,
                &format!("{outcome:?}; next check in {}s", delay.as_secs()),
            );
            std::thread::sleep(delay);
        }
    });
}

async fn try_update(
    app: &AppHandle,
    log_path: &Path,
) -> Result<UpdateOutcome, Box<dyn std::error::Error>> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(UpdateOutcome::UpToDate);
    };
    log(log_path, &format!("update available: {}", update.version));
    let in_game = app
        .state::<AppState>()
        .status
        .lock()
        .map_or(true, |s| s.state == GameState::InGame);
    if in_game {
        return Ok(UpdateOutcome::DeferredInGame);
    }
    update.download_and_install(|_, _| {}, || {}).await?;
    log(log_path, "update installed, restarting");
    app.restart();
}

fn log(path: &Path, message: &str) {
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
        let _ = std::fs::remove_file(path);
    }
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "[{secs}] {message}");
    }
}
