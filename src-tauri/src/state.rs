//! Shared application state.

use lgr_core::store::SessionStore;
use lgr_engine::{GameState, Status};
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

pub struct AppState {
    pub store: SessionStore,
    pub ffmpeg: Option<PathBuf>,
    pub status: Arc<Mutex<Status>>,
    pub stop: Arc<AtomicBool>,
    pub monitor_thread: Mutex<Option<JoinHandle<()>>>,
}

impl AppState {
    pub fn new(store: SessionStore, ffmpeg: Option<PathBuf>) -> Self {
        let status = Status {
            state: GameState::Idle,
            recording: false,
            ffmpeg_available: ffmpeg.is_some(),
        };
        Self {
            store,
            ffmpeg,
            status: Arc::new(Mutex::new(status)),
            stop: Arc::new(AtomicBool::new(false)),
            monitor_thread: Mutex::new(None),
        }
    }
}
