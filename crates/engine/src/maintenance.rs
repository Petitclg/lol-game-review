//! Background housekeeping run while no game is running: clip compression and the
//! retention cap.

use crate::storage::{enforce_retention, Reclaimed};
use lgr_core::settings::Settings;
use lgr_core::store::{SessionStore, StoreError};
use lgr_media::Encoder;
use std::path::{Path, PathBuf};

/// A clip is only replaced when the result is clearly smaller.
const MIN_SAVING_RATIO: f64 = 0.92;

/// Re-encodes a clip file; abstracted so maintenance can be tested.
pub trait ClipCompressor {
    fn compress(&self, input: &Path, output: &Path) -> Result<(), String>;
}

pub struct FfmpegCompressor {
    pub ffmpeg: PathBuf,
    pub encoder: Encoder,
}

impl ClipCompressor for FfmpegCompressor {
    fn compress(&self, input: &Path, output: &Path) -> Result<(), String> {
        lgr_media::compress_clip(&self.ffmpeg, self.encoder, input, output)
            .map_err(|e| e.to_string())
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MaintenanceReport {
    pub clips_compressed: usize,
    pub bytes_saved: u64,
    pub reclaimed: Vec<Reclaimed>,
}

/// Compresses the clips not yet processed (newest games first), then applies the
/// retention cap. `idle` is checked before each clip so a game starting stops the work
/// promptly.
pub fn run_maintenance(
    store: &SessionStore,
    settings: &Settings,
    compressor: Option<&dyn ClipCompressor>,
    idle: &dyn Fn() -> bool,
) -> Result<MaintenanceReport, StoreError> {
    let mut report = MaintenanceReport::default();
    if let (true, Some(compressor)) = (settings.compress_clips, compressor) {
        'sessions: for meta in store.list()? {
            let done = store.load_compressed(&meta.id)?;
            for clip in store.load_clips(&meta.id)? {
                let Some(file) = clip.file.as_deref().map(Path::new) else {
                    continue;
                };
                if done.contains(&clip.id) || !file.is_file() {
                    continue;
                }
                if !idle() {
                    break 'sessions;
                }
                compress_one(store, compressor, &meta.id, &clip.id, file, &mut report)?;
            }
        }
    }
    if idle() {
        if let Some(cap) = settings.retention_bytes() {
            report.reclaimed = enforce_retention(store, cap)?;
        }
    }
    Ok(report)
}

fn compress_one(
    store: &SessionStore,
    compressor: &dyn ClipCompressor,
    session_id: &str,
    clip_id: &str,
    file: &Path,
    report: &mut MaintenanceReport,
) -> Result<(), StoreError> {
    let tmp = file.with_extension("lgrtmp.mp4");
    let original = std::fs::metadata(file).map_or(0, |m| m.len());
    match compressor.compress(file, &tmp) {
        Ok(()) => {
            let smaller = std::fs::metadata(&tmp).map_or(u64::MAX, |m| m.len());
            if (smaller as f64) <= (original as f64) * MIN_SAVING_RATIO
                && std::fs::rename(&tmp, file).is_ok()
            {
                report.clips_compressed += 1;
                report.bytes_saved += original - smaller;
                store.mark_compressed(session_id, clip_id)?;
            } else {
                // Not worth replacing (or the file is in use): never retry this clip
                // when it is simply not compressible; a locked file is retried later.
                let replaced_failed = smaller <= ((original as f64) * MIN_SAVING_RATIO) as u64;
                let _ = std::fs::remove_file(&tmp);
                if !replaced_failed {
                    store.mark_compressed(session_id, clip_id)?;
                }
            }
        }
        Err(_) => {
            // The encoder failed: leave the clip as is and retry on a later pass.
            let _ = std::fs::remove_file(&tmp);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgr_core::session::{Clip, Session};
    use std::cell::{Cell, RefCell};
    use std::fs;

    struct Fake {
        /// Output size as a fraction of the input size; `None` = the encoder fails.
        ratio: Option<f64>,
        calls: RefCell<Vec<String>>,
    }

    impl ClipCompressor for Fake {
        fn compress(&self, input: &Path, output: &Path) -> Result<(), String> {
            self.calls
                .borrow_mut()
                .push(input.file_name().unwrap().to_string_lossy().into_owned());
            let Some(r) = self.ratio else {
                return Err("encoder failed".into());
            };
            let len = fs::metadata(input).unwrap().len() as f64;
            fs::write(output, vec![1u8; (len * r) as usize]).map_err(|e| e.to_string())
        }
    }

    fn fake(ratio: Option<f64>) -> Fake {
        Fake {
            ratio,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn clip(id: &str, file: &Path) -> Clip {
        Clip {
            id: id.into(),
            theme: lgr_core::events::Theme::Kill,
            label: "Kill".into(),
            game_time_secs: 1.0,
            start_secs: 0.0,
            end_secs: 1.0,
            file: Some(file.to_string_lossy().into_owned()),
            players: vec![],
            involved: vec![],
            my_gold: None,
        }
    }

    fn add_game(
        store: &SessionStore,
        id: &str,
        started: u64,
        clip_sizes: &[usize],
    ) -> Vec<PathBuf> {
        store
            .save_session(&Session {
                id: id.into(),
                started_at_ms: started,
                player_aliases: vec![],
                video_anchor_secs: 0.0,
                recording_file: None,
                last_game_time_secs: 600.0,
                events: vec![],
                snapshots: vec![],
            })
            .unwrap();
        let mut files = Vec::new();
        let mut clips = Vec::new();
        for (i, size) in clip_sizes.iter().enumerate() {
            let f = store.clips_dir(id).unwrap().join(format!("kill-{i}.mp4"));
            fs::write(&f, vec![0u8; *size]).unwrap();
            clips.push(clip(&format!("kill-{i}"), &f));
            files.push(f);
        }
        store.save_clips(id, &clips).unwrap();
        files
    }

    fn only_compression() -> Settings {
        Settings {
            retention_gb: None,
            ..Settings::default()
        }
    }

    #[test]
    fn clips_are_replaced_by_their_smaller_version_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let files = add_game(&store, "a", 1, &[10_000, 20_000]);
        let c = fake(Some(0.5));

        let r = run_maintenance(&store, &only_compression(), Some(&c), &|| true).unwrap();
        assert_eq!((r.clips_compressed, r.bytes_saved), (2, 15_000));
        assert_eq!(fs::metadata(&files[0]).unwrap().len(), 5_000);
        assert!(!files[0].with_extension("lgrtmp.mp4").exists());

        let again = run_maintenance(&store, &only_compression(), Some(&c), &|| true).unwrap();
        assert_eq!(again, MaintenanceReport::default());
        assert_eq!(
            c.calls.borrow().len(),
            2,
            "already compressed clips are skipped"
        );
    }

    #[test]
    fn a_barely_smaller_result_keeps_the_original_and_is_not_retried() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let files = add_game(&store, "a", 1, &[10_000]);
        let c = fake(Some(0.97));
        let r = run_maintenance(&store, &only_compression(), Some(&c), &|| true).unwrap();
        assert_eq!(r.clips_compressed, 0);
        assert_eq!(fs::metadata(&files[0]).unwrap().len(), 10_000);
        run_maintenance(&store, &only_compression(), Some(&c), &|| true).unwrap();
        assert_eq!(c.calls.borrow().len(), 1);
    }

    #[test]
    fn an_encoder_failure_leaves_the_clip_and_is_retried_later() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let files = add_game(&store, "a", 1, &[10_000]);
        let failing = fake(None);
        run_maintenance(&store, &only_compression(), Some(&failing), &|| true).unwrap();
        assert_eq!(fs::metadata(&files[0]).unwrap().len(), 10_000);
        let working = fake(Some(0.5));
        let r = run_maintenance(&store, &only_compression(), Some(&working), &|| true).unwrap();
        assert_eq!(r.clips_compressed, 1);
    }

    #[test]
    fn nothing_is_compressed_when_the_setting_is_off_or_there_is_no_encoder() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, &[10_000]);
        let c = fake(Some(0.5));
        let off = Settings {
            compress_clips: false,
            retention_gb: None,
            ..Settings::default()
        };
        assert_eq!(
            run_maintenance(&store, &off, Some(&c), &|| true)
                .unwrap()
                .clips_compressed,
            0
        );
        assert_eq!(
            run_maintenance(&store, &only_compression(), None, &|| true)
                .unwrap()
                .clips_compressed,
            0
        );
        assert!(c.calls.borrow().is_empty());
    }

    #[test]
    fn a_game_starting_stops_the_work_immediately() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, &[10_000, 10_000, 10_000]);
        let c = fake(Some(0.5));
        let budget = Cell::new(1);
        let idle = || {
            let left = budget.get();
            budget.set(left - 1);
            left > 0
        };
        let r = run_maintenance(&store, &only_compression(), Some(&c), &idle).unwrap();
        assert_eq!(
            r.clips_compressed, 1,
            "stopped as soon as a game was detected"
        );
    }

    #[test]
    fn retention_runs_after_compression_and_only_when_idle() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        for (id, started) in [("old", 1u64), ("new", 2)] {
            add_game(&store, id, started, &[10_000]);
            fs::write(store.recording_path(id).unwrap(), vec![0u8; 500_000]).unwrap();
        }
        let capped = Settings {
            compress_clips: false,
            retention_gb: Some(1),
            ..Settings::default()
        };
        // 1 GB is far above the test data: nothing to reclaim
        assert!(run_maintenance(&store, &capped, None, &|| true)
            .unwrap()
            .reclaimed
            .is_empty());
        // during a game the retention pass is skipped entirely
        let r = run_maintenance(&store, &capped, None, &|| false).unwrap();
        assert!(r.reclaimed.is_empty());
    }
}
