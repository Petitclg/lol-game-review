//! Application entry point: window, tray icon and background services.

mod commands;
mod runtime;
mod state;
mod updater;

use lgr_core::store::SessionStore;
use state::AppState;
use std::sync::atomic::Ordering;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

fn show_main_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Ouvrir", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quitter", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;
    let mut tray = TrayIconBuilder::new()
        .tooltip("LoL Game Review")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

pub fn run() {
    let sandbox = lgr_engine::sandbox::current_sandbox();
    let mut builder = tauri::Builder::default();
    if let Some(dir) = &sandbox {
        // Isolates the WebView from the installed app's, so debugging flags apply and
        // no state is shared.
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", dir.join("webview"));
    } else {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app);
        }));
    }
    let app = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(move |app| {
            let data_dir = match &sandbox {
                Some(dir) => dir.clone(),
                None => app.path().app_local_data_dir()?,
            };
            let ffmpeg = lgr_media::locate_ffmpeg(&runtime_dirs(app.handle()));
            let state = AppState::new(SessionStore::new(data_dir.join("sessions")), ffmpeg);
            if sandbox.is_none() {
                let handle = runtime::spawn_monitor(app.handle().clone(), &state);
                *state.monitor_thread.lock().map_err(|e| e.to_string())? = handle;
            } else {
                // The configured asset scope only covers the real data folder.
                let sessions = data_dir.join("sessions");
                let _ = std::fs::create_dir_all(&sessions);
                let _ = app.asset_protocol_scope().allow_directory(&sessions, true);
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_title("LoL Game Review (BAC À SABLE : aucun enregistrement)");
                }
            }
            app.manage(state);
            build_tray(app.handle())?;
            if !cfg!(debug_assertions) && sandbox.is_none() {
                updater::start(
                    app.handle().clone(),
                    data_dir.join("logs").join("updater.log"),
                );
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Closing the window keeps the app (and game monitoring) alive in the tray.
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_account,
            commands::list_games,
            commands::generate_clips,
            commands::get_clips,
            commands::delete_recording,
            commands::get_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building the application");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            let state = app.state::<AppState>();
            state.stop.store(true, Ordering::Relaxed);
            let handle = state.monitor_thread.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = handle {
                let _ = handle.join();
            }
        }
    });
}

/// Directories where a bundled ffmpeg may live: next to the executable.
fn runtime_dirs(app: &AppHandle) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            dirs.push(dir.to_path_buf());
        }
    }
    if let Ok(res) = app.path().resource_dir() {
        dirs.push(res);
    }
    dirs
}
