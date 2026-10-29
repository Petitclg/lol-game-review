//! Shared application state.

use lgr_core::settings::SettingsStore;
use lgr_core::store::SessionStore;
use lgr_engine::{GameState, Status};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct AppState {
    pub store: SessionStore,
    pub settings: SettingsStore,
    pub ffmpeg: Option<PathBuf>,
    pub status: Arc<Mutex<Status>>,
    pub stop: Arc<AtomicBool>,
    pub monitor_thread: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn new(store: SessionStore, settings: SettingsStore, ffmpeg: Option<PathBuf>) -> Self {
        let status = Status {
            state: GameState::Idle,
            recording: false,
            ffmpeg_available: ffmpeg.is_some(),
        };
        Self {
            store,
            settings,
            ffmpeg,
            status: Arc::new(Mutex::new(status)),
            stop: Arc::new(AtomicBool::new(false)),
            monitor_thread: Mutex::new(None),
        }
    }
}
