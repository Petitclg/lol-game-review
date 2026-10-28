//! Clip generation from a stored session and its recording.

use lgr_core::clips::ClipConfig;
use lgr_core::session::{build_clips, Clip};
use lgr_core::store::{SessionStore, StoreError};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ClipError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the source recording is not available")]
    NoRecording,
    #[error("no clip could be extracted")]
    NothingExtracted,
}

/// Cuts a clip from the recording; abstracted so generation can be tested.
pub trait ClipCutter {
    fn cut(
        &self,
        input: &Path,
        start_secs: f64,
        end_secs: f64,
        output: &Path,
    ) -> Result<(), String>;
}

pub struct FfmpegCutter {
    pub ffmpeg: std::path::PathBuf,
}

impl ClipCutter for FfmpegCutter {
    fn cut(&self, input: &Path, start: f64, end: f64, output: &Path) -> Result<(), String> {
        lgr_media::cut_clip(&self.ffmpeg, input, start, end, output).map_err(|e| e.to_string())
    }
}

/// Generates (or completes) the clips of a session. Existing clip files are reused.
/// `progress(done, total)` is called after each clip.
pub fn generate_clips(
    store: &SessionStore,
    cutter: &dyn ClipCutter,
    session_id: &str,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Clip>, ClipError> {
    let session = store.load_session(session_id)?;
    let recording = store.recording_path(session_id)?;
    let clips_dir = store.clips_dir(session_id)?;
    let mut clips = build_clips(&session, ClipConfig::default());
    let total = clips.len();
    std::fs::create_dir_all(&clips_dir).map_err(StoreError::from)?;

    let need_recording = clips
        .iter()
        .any(|c| !clips_dir.join(format!("{}.mp4", c.id)).is_file());
    if need_recording && !recording.is_file() {
        return Err(ClipError::NoRecording);
    }

    let mut extracted = 0;
    for (i, clip) in clips.iter_mut().enumerate() {
        let out = clips_dir.join(format!("{}.mp4", clip.id));
        let ready = out.is_file()
            || cutter
                .cut(&recording, clip.start_secs, clip.end_secs, &out)
                .is_ok();
        if ready {
            clip.file = Some(out.to_string_lossy().into_owned());
            extracted += 1;
        }
        progress(i + 1, total);
    }
    if total > 0 && extracted == 0 {
        return Err(ClipError::NothingExtracted);
    }
    store.save_clips(session_id, &clips)?;
    Ok(clips)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgr_core::events::RawLiveEvent;
    use lgr_core::session::Session;
    use std::cell::RefCell;

    struct FakeCutter {
        calls: RefCell<Vec<(f64, f64)>>,
        fail: bool,
    }

    impl ClipCutter for FakeCutter {
        fn cut(&self, _: &Path, s: f64, e: f64, out: &Path) -> Result<(), String> {
            self.calls.borrow_mut().push((s, e));
            if self.fail {
                return Err("boom".into());
            }
            std::fs::write(out, b"mp4").map_err(|e| e.to_string())
        }
    }

    fn cutter(fail: bool) -> FakeCutter {
        FakeCutter {
            calls: RefCell::new(vec![]),
            fail,
        }
    }

    fn kill(id: u32, t: f64) -> RawLiveEvent {
        RawLiveEvent {
            event_id: id,
            event_name: "ChampionKill".into(),
            event_time: t,
            killer_name: Some("Me".into()),
            victim_name: Some("X".into()),
            assisters: vec![],
            dragon_type: None,
        }
    }

    fn setup(with_recording: bool) -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store
            .save_session(&Session {
                id: "s".into(),
                started_at_ms: 0,
                player_aliases: vec!["Me".into()],
                video_anchor_secs: 0.0,
                recording_file: None,
                last_game_time_secs: 500.0,
                events: vec![kill(1, 100.0), kill(2, 300.0)],
                snapshots: vec![],
            })
            .unwrap();
        if with_recording {
            std::fs::write(store.recording_path("s").unwrap(), b"mkv").unwrap();
        }
        (dir, store)
    }

    #[test]
    fn generates_files_and_reports_progress() {
        let (_d, store) = setup(true);
        let c = cutter(false);
        let mut seen = vec![];
        let clips = generate_clips(&store, &c, "s", |d, t| seen.push((d, t))).unwrap();
        assert_eq!(clips.len(), 2);
        assert!(clips
            .iter()
            .all(|c| c.file.as_ref().is_some_and(|f| Path::new(f).is_file())));
        assert_eq!(seen, [(1, 2), (2, 2)]);
        assert_eq!(store.load_clips("s").unwrap(), clips);
    }

    #[test]
    fn existing_clip_files_are_not_recut() {
        let (_d, store) = setup(true);
        let c = cutter(false);
        generate_clips(&store, &c, "s", |_, _| {}).unwrap();
        let c2 = cutter(false);
        generate_clips(&store, &c2, "s", |_, _| {}).unwrap();
        assert!(c2.calls.borrow().is_empty());
    }

    #[test]
    fn works_without_recording_once_clips_exist() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        store.delete_recording("s").unwrap();
        assert_eq!(
            generate_clips(&store, &cutter(true), "s", |_, _| {})
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn missing_recording_is_an_error() {
        let (_d, store) = setup(false);
        let err = generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap_err();
        assert!(matches!(err, ClipError::NoRecording));
    }

    #[test]
    fn all_cuts_failing_is_an_error() {
        let (_d, store) = setup(true);
        let err = generate_clips(&store, &cutter(true), "s", |_, _| {}).unwrap_err();
        assert!(matches!(err, ClipError::NothingExtracted));
    }

    #[test]
    fn unknown_session_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let err = generate_clips(&store, &cutter(false), "nope", |_, _| {}).unwrap_err();
        assert!(matches!(err, ClipError::Store(StoreError::NotFound)));
    }
}
