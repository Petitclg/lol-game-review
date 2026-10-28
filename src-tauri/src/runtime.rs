//! Background game monitor thread and the production recorder.

use crate::state::AppState;
use lgr_engine::{ActiveRecording, Monitor, RecorderFactory, Status};
use lgr_live::LiveClient;
use lgr_media::{detect_encoder, Encoder, Recording};
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

struct FfmpegRecorder {
    ffmpeg: Option<PathBuf>,
    encoder: Encoder,
}

struct Rec(Recording);

impl ActiveRecording for Rec {
    fn stop(self: Box<Self>) {
        let _ = self.0.stop();
    }
}

impl RecorderFactory for FfmpegRecorder {
    fn available(&self) -> bool {
        self.ffmpeg.is_some()
    }

    fn start(&self, output: &Path) -> Result<Box<dyn ActiveRecording>, String> {
        let ffmpeg = self.ffmpeg.as_ref().ok_or("ffmpeg not found")?;
        let rec = Recording::start(ffmpeg, self.encoder, output).map_err(|e| e.to_string())?;
        Ok(Box::new(Rec(rec)))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

/// Starts the monitor loop; it stops (finalizing any recording) when `state.stop` is set.
pub fn spawn_monitor(app: AppHandle, state: &AppState) -> Option<JoinHandle<()>> {
    let live = LiveClient::new().ok()?;
    let ffmpeg = state.ffmpeg.clone();
    let store = state.store.clone();
    let stop = Arc::clone(&state.stop);
    let status = Arc::clone(&state.status);

    Some(thread::spawn(move || {
        let encoder = ffmpeg.as_deref().map_or(Encoder::X264, detect_encoder);
        let on_status = Arc::new(move |s: &Status| {
            if let Ok(mut current) = status.lock() {
                *current = s.clone();
            }
            let _ = app.emit("status-changed", s);
        });
        let mut monitor = Monitor::new(live, FfmpegRecorder { ffmpeg, encoder }, store, on_status);

        while !stop.load(Ordering::Relaxed) {
            monitor.tick(now_ms());
            let mut waited = Duration::ZERO;
            let interval = monitor.next_interval();
            while waited < interval && !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(250));
                waited += Duration::from_millis(250);
            }
        }
        monitor.shutdown();
    }))
}
