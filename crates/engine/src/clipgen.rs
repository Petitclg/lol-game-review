//! Clip generation from a stored session and its recording.

use lgr_core::clips::ClipConfig;
use lgr_core::loading::{anchor_from_loading_end, frame_diffs, loading_end_secs};
use lgr_core::session::{build_clips_with, Casts, Clip, Session};
use lgr_core::store::{SessionStore, StoreError, UltimateRecord};
use lgr_core::ultimate::{detect_ultimates, ANALYSIS_VERSION};
use serde::Serialize;
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

/// What the HUD analysis found in a recording.
#[derive(Debug, Clone, PartialEq)]
pub enum UltimateOutcome {
    /// The game interface was located; casts are in video seconds.
    Recognized {
        casts_video_secs: Vec<f64>,
        /// Casts of summoner spell one and two.
        summoner_casts_video_secs: [Vec<f64>; 2],
        hud_scale: f64,
    },
    /// The interface could not be located (unusual scale, layout, or hidden HUD).
    Unrecognized,
}

/// Finds the local player's ultimate casts in a recording; abstracted for tests.
pub trait UltimateDetector {
    fn analyse(&self, recording: &Path) -> Result<UltimateOutcome, String>;

    /// Video time (secs) at which the game starts, when the recording begins with a
    /// loading screen; `Ok(None)` when it does not. An error means "could not tell".
    fn loading_end_secs(&self, _recording: &Path) -> Result<Option<f64>, String> {
        Err("video analysis is disabled".into())
    }
}

/// Reads the cooldown digits of the ability icons in the recorded HUD, after locating
/// the HUD scale that fits the recording.
pub struct HudUltimateDetector {
    pub ffmpeg: std::path::PathBuf,
}

impl UltimateDetector for HudUltimateDetector {
    fn analyse(&self, recording: &Path) -> Result<UltimateOutcome, String> {
        let scales = lgr_media::SCALES;
        let rows = lgr_media::extract_digit_ratios(&self.ffmpeg, recording, &scales)
            .map_err(|e| e.to_string())?;
        // Where the game starts matters for the summoner spells; if it cannot be told the
        // whole video is read.
        let game_start = self
            .loading_end_secs(recording)
            .ok()
            .flatten()
            .map_or(0, |secs| secs as usize);
        Ok(match detect_ultimates(&rows, &scales, game_start) {
            Some(d) => UltimateOutcome::Recognized {
                casts_video_secs: d.casts_video_secs,
                summoner_casts_video_secs: d.summoner_casts_video_secs,
                hud_scale: scales[d.candidate],
            },
            None => UltimateOutcome::Unrecognized,
        })
    }

    fn loading_end_secs(&self, recording: &Path) -> Result<Option<f64>, String> {
        let raw =
            lgr_media::extract_start_frames(&self.ffmpeg, recording, lgr_media::START_SCAN_SECS)
                .map_err(|e| e.to_string())?;
        let diffs = frame_diffs(&raw, lgr_media::START_FRAME_LEN);
        Ok(loading_end_secs(&diffs))
    }
}

/// Detector for callers that do not analyse the video.
pub struct NoUltimates;

impl UltimateDetector for NoUltimates {
    fn analyse(&self, _: &Path) -> Result<UltimateOutcome, String> {
        Err("video analysis is disabled".into())
    }
}

/// Sessions saved before the anchor ignored the frozen clock of the loading screen have
/// an anchor near zero even though their recording starts earlier; larger anchors come
/// from a recording that began mid-game and are right.
const SUSPECT_ANCHOR_SECS: f64 = 5.0;

/// Checks the video anchor of a session saved by an older version against its
/// recording and corrects it. When the anchor changes, everything derived from it (the
/// ultimate analysis and the generated clips) is discarded, as it is mistimed.
/// Nothing is changed when the recording is missing or cannot be analysed.
fn verified_session(
    store: &SessionStore,
    detector: &dyn UltimateDetector,
    mut session: Session,
) -> Session {
    if session.anchor_verified || session.video_anchor_secs.abs() > SUSPECT_ANCHOR_SECS {
        return session;
    }
    let Ok(recording) = store.recording_path(&session.id) else {
        return session;
    };
    if !recording.is_file() {
        return session;
    }
    let Ok(loading_end) = detector.loading_end_secs(&recording) else {
        return session;
    };
    let corrected = loading_end.map(anchor_from_loading_end);
    session.anchor_verified = true;
    if let Some(anchor) = corrected {
        session.video_anchor_secs = anchor;
    }
    if store.save_session(&session).is_ok() && corrected.is_some() {
        let _ = store.delete_ultimates(&session.id);
        let _ = store.delete_clips(&session.id);
    }
    session
}

/// Whether a saved analysis was made by the current detection (including the summoner
/// spells); older ones are analysed again when the video is still there.
fn is_current(record: &UltimateRecord) -> bool {
    record.summoner_casts.is_some() && record.analysis_version >= ANALYSIS_VERSION
}

/// Casts of a session read from its recorded HUD, in game time. The outcome is cached
/// (including "interface not recognised"), so the video is analysed once; a failed
/// analysis is not cached and is retried next time.
///
/// An analysis saved by an older version (no summoner spells, possibly another
/// calibration) is redone when the video is still there. If its ultimates changed, the
/// ultimate clip files cut from the old times are removed so they are cut again.
fn hud_casts(store: &SessionStore, detector: &dyn UltimateDetector, session: &Session) -> Casts {
    let cached = store.load_ultimates(&session.id).ok().flatten();
    if let Some(record) = cached.as_ref().filter(|r| is_current(r)) {
        return record.to_casts();
    }
    let fallback = cached
        .as_ref()
        .map(UltimateRecord::to_casts)
        .unwrap_or_default();
    let Ok(recording) = store.recording_path(&session.id) else {
        return fallback;
    };
    if !recording.is_file() {
        return fallback;
    }
    let to_game_time = |video: Vec<f64>| -> Vec<f64> {
        video
            .into_iter()
            .map(|v| v + session.video_anchor_secs)
            .collect()
    };
    let record = match detector.analyse(&recording) {
        Ok(UltimateOutcome::Recognized {
            casts_video_secs,
            summoner_casts_video_secs: [first, second],
            hud_scale,
        }) => UltimateRecord {
            recognized: true,
            hud_scale: Some(hud_scale),
            casts: to_game_time(casts_video_secs),
            summoner_casts: Some([to_game_time(first), to_game_time(second)]),
            analysis_version: ANALYSIS_VERSION,
        },
        Ok(UltimateOutcome::Unrecognized) => UltimateRecord {
            recognized: false,
            hud_scale: None,
            casts: Vec::new(),
            summoner_casts: Some([Vec::new(), Vec::new()]),
            analysis_version: ANALYSIS_VERSION,
        },
        Err(_) => return fallback,
    };
    if cached.is_some_and(|old| old.casts != record.casts) {
        let _ = store.delete_clip_files(&session.id, "ultimate-");
    }
    let _ = store.save_ultimates(&session.id, &record);
    record.to_casts()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UltimateState {
    Ok,
    Unrecognized,
    NotAnalyzed,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UltimateStatus {
    pub state: UltimateState,
    pub casts: usize,
    pub hud_scale: Option<f64>,
}

/// Outcome of the ultimate analysis of a session, for display.
pub fn ultimate_status(
    store: &SessionStore,
    session_id: &str,
) -> Result<UltimateStatus, ClipError> {
    Ok(match store.load_ultimates(session_id)? {
        None => UltimateStatus {
            state: UltimateState::NotAnalyzed,
            casts: 0,
            hud_scale: None,
        },
        Some(r) if r.recognized => UltimateStatus {
            state: UltimateState::Ok,
            casts: r.casts.len(),
            hud_scale: r.hud_scale,
        },
        Some(_) => UltimateStatus {
            state: UltimateState::Unrecognized,
            casts: 0,
            hud_scale: None,
        },
    })
}

/// Clips of a session with metadata recomputed from the stored timeline, so clips
/// generated by an older version gain new information (e.g. involved players).
/// A generated file is kept only when its id and window still match.
/// Returns an empty list when no clips were generated yet.
pub fn load_clips(store: &SessionStore, session_id: &str) -> Result<Vec<Clip>, ClipError> {
    let stored = store.load_clips(session_id)?;
    if stored.is_empty() {
        return Ok(stored);
    }
    let session = store.load_session(session_id)?;
    let casts = store
        .load_ultimates(session_id)?
        .map(|r| r.to_casts())
        .unwrap_or_default();
    let mut clips = build_clips_with(&session, ClipConfig::default(), &casts);
    for clip in &mut clips {
        clip.file = stored
            .iter()
            .find(|s| s.id == clip.id && (s.start_secs - clip.start_secs).abs() < 1.0)
            .and_then(|s| s.file.clone())
            .filter(|f| Path::new(f).is_file());
    }
    Ok(clips)
}

/// Generates (or completes) the clips of a session without analysing the video for
/// ultimates. Clip files that are still current are reused.
pub fn generate_clips(
    store: &SessionStore,
    cutter: &dyn ClipCutter,
    session_id: &str,
    progress: impl FnMut(usize, usize),
) -> Result<Vec<Clip>, ClipError> {
    generate_clips_with(store, cutter, &NoUltimates, session_id, progress)
}

/// Window tolerance (secs) under which a stored clip still matches the one to cut.
const SAME_WINDOW_SECS: f64 = 0.5;

/// Cuts `clip` again over its existing file. The new clip is cut aside and only then
/// replaces the old one, so a failed cut loses nothing. The re-encoded copy of the old
/// clip (if any) is gone, so it is eligible for compression again.
fn replace_clip(
    store: &SessionStore,
    cutter: &dyn ClipCutter,
    session_id: &str,
    clip: &Clip,
    recording: &Path,
    out: &Path,
) -> bool {
    let aside = out.with_extension("new.mp4");
    let cut = cutter.cut(recording, clip.start_secs, clip.end_secs, &aside);
    if cut.is_ok() && std::fs::rename(&aside, out).is_ok() {
        let _ = store.unmark_compressed(session_id, &clip.id);
        return true;
    }
    let _ = std::fs::remove_file(&aside);
    // the old file is still there and still plays
    true
}

/// Generates (or completes) the clips of a session, including the local player's
/// ultimates and Flashs found by `detector`.
///
/// A clip file is reused only when its window is the one to cut now: after a change of
/// the clip lengths, of the video anchor or of the HUD analysis the clip is cut again,
/// from a temporary file so that the old clip survives a failed cut. Without the source
/// video a clip that is out of date is kept as it is.
/// `progress(done, total)` is called after each clip.
pub fn generate_clips_with(
    store: &SessionStore,
    cutter: &dyn ClipCutter,
    detector: &dyn UltimateDetector,
    session_id: &str,
    mut progress: impl FnMut(usize, usize),
) -> Result<Vec<Clip>, ClipError> {
    let session = verified_session(store, detector, store.load_session(session_id)?);
    let recording = store.recording_path(session_id)?;
    let clips_dir = store.clips_dir(session_id)?;
    let casts = hud_casts(store, detector, &session);
    let stored = store.load_clips(session_id).unwrap_or_default();
    let mut clips = build_clips_with(&session, ClipConfig::default(), &casts);
    let total = clips.len();
    std::fs::create_dir_all(&clips_dir).map_err(StoreError::from)?;

    let need_recording = clips
        .iter()
        .any(|c| !clips_dir.join(format!("{}.mp4", c.id)).is_file());
    if need_recording && !recording.is_file() {
        return Err(ClipError::NoRecording);
    }
    let can_cut = recording.is_file();

    let mut extracted = 0;
    for (i, clip) in clips.iter_mut().enumerate() {
        let out = clips_dir.join(format!("{}.mp4", clip.id));
        let exists = out.is_file();
        let current = exists
            && stored.iter().any(|s| {
                s.id == clip.id
                    && (s.start_secs - clip.start_secs).abs() < SAME_WINDOW_SECS
                    && (s.end_secs - clip.end_secs).abs() < SAME_WINDOW_SECS
            });
        let ready = if current || (exists && !can_cut) {
            true
        } else if exists {
            replace_clip(store, cutter, &session.id, clip, &recording, &out)
        } else {
            cutter
                .cut(&recording, clip.start_secs, clip.end_secs, &out)
                .is_ok()
        };
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
            ..RawLiveEvent::default()
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
                anchor_verified: false,
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

    /// Shifts the video anchor of the test session, which moves every clip window.
    fn shift_windows(store: &SessionStore, secs: f64) {
        let mut s = store.load_session("s").unwrap();
        s.video_anchor_secs = secs;
        store.save_session(&s).unwrap();
    }

    #[test]
    fn a_clip_whose_window_changed_is_cut_again_over_its_file() {
        let (_d, store) = setup(true);
        let first = generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let file = first[0].file.clone().unwrap();
        std::fs::write(&file, b"old").unwrap();

        shift_windows(&store, 3.0);
        let c = cutter(false);
        let second = generate_clips(&store, &c, "s", |_, _| {}).unwrap();

        assert_eq!(c.calls.borrow().len(), 2, "both windows moved");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            b"mp4",
            "the file was replaced"
        );
        assert!((second[0].start_secs - (first[0].start_secs - 3.0)).abs() < 1e-9);
        assert_eq!(store.load_clips("s").unwrap(), second);
        let leftovers = std::fs::read_dir(store.clips_dir("s").unwrap())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".new."))
            .count();
        assert_eq!(leftovers, 0, "no temporary file is left behind");

        let c3 = cutter(false);
        generate_clips(&store, &c3, "s", |_, _| {}).unwrap();
        assert!(c3.calls.borrow().is_empty(), "now current again");
    }

    #[test]
    fn a_failed_recut_keeps_the_old_clip() {
        let (_d, store) = setup(true);
        let first = generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let file = first[0].file.clone().unwrap();
        std::fs::write(&file, b"old").unwrap();

        shift_windows(&store, 3.0);
        let clips = generate_clips(&store, &cutter(true), "s", |_, _| {}).unwrap();

        assert_eq!(std::fs::read(&file).unwrap(), b"old");
        assert!(clips[0].file.is_some());
        assert!(!Path::new(&file).with_extension("new.mp4").exists());
    }

    #[test]
    fn a_recut_clip_can_be_compressed_again() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        store.mark_compressed("s", "kill-0").unwrap();
        store.mark_compressed("s", "kill-1").unwrap();

        shift_windows(&store, 3.0);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        assert!(store.load_compressed("s").unwrap().is_empty());
    }

    #[test]
    fn out_of_date_clips_are_kept_when_the_source_video_is_gone() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        store.delete_recording("s").unwrap();
        shift_windows(&store, 3.0);

        let c = cutter(false);
        let clips = generate_clips(&store, &c, "s", |_, _| {}).unwrap();
        assert!(c.calls.borrow().is_empty());
        assert!(clips.iter().all(|c| c.file.is_some()));
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
    fn load_clips_refreshes_metadata_but_keeps_generated_files() {
        let (_d, store) = setup(true);
        let generated = generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        assert!(generated[0].involved.is_empty());

        // The timeline gains player data after the clips were generated.
        let mut session = store.load_session("s").unwrap();
        session.snapshots = vec![lgr_core::session::Snapshot {
            game_time_secs: 0.0,
            players: vec![lgr_core::session::PlayerSnapshot {
                name: "Me#EUW".into(),
                champion: "Ahri".into(),
                team: "ORDER".into(),
                is_me: true,
                level: 5,
                kills: 0,
                deaths: 0,
                assists: 0,
                items: vec![],
                spells: vec![],
                spell_keys: vec![],
            }],
            my_gold: Some(123.0),
        }];
        store.save_session(&session).unwrap();

        let clips = load_clips(&store, "s").unwrap();
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].involved[0].name, "Me#EUW");
        assert_eq!(clips[0].my_gold, Some(123.0));
        assert_eq!(clips[0].file, generated[0].file);
    }

    #[test]
    fn load_clips_drops_files_whose_window_changed() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let mut stored = store.load_clips("s").unwrap();
        stored[0].start_secs += 30.0;
        store.save_clips("s", &stored).unwrap();
        let clips = load_clips(&store, "s").unwrap();
        assert!(clips[0].file.is_none());
        assert!(clips[1].file.is_some());
    }

    #[test]
    fn load_clips_is_empty_before_generation() {
        let (_d, store) = setup(true);
        assert!(load_clips(&store, "s").unwrap().is_empty());
    }

    struct FakeDetector {
        outcome: Result<UltimateOutcome, String>,
        calls: std::cell::Cell<u32>,
    }

    impl UltimateDetector for FakeDetector {
        fn analyse(&self, _: &Path) -> Result<UltimateOutcome, String> {
            self.calls.set(self.calls.get() + 1);
            self.outcome.clone()
        }
    }

    fn detector(video_secs: &[f64], fail: bool) -> FakeDetector {
        detector_with_summoners(video_secs, [vec![], vec![]], fail)
    }

    fn detector_with_summoners(
        video_secs: &[f64],
        summoners: [Vec<f64>; 2],
        fail: bool,
    ) -> FakeDetector {
        FakeDetector {
            outcome: if fail {
                Err("no ffmpeg".into())
            } else {
                Ok(UltimateOutcome::Recognized {
                    casts_video_secs: video_secs.to_vec(),
                    summoner_casts_video_secs: summoners,
                    hud_scale: 1.0,
                })
            },
            calls: std::cell::Cell::new(0),
        }
    }

    fn unrecognizing() -> FakeDetector {
        FakeDetector {
            outcome: Ok(UltimateOutcome::Unrecognized),
            calls: std::cell::Cell::new(0),
        }
    }

    #[test]
    fn ultimates_are_detected_converted_to_game_time_and_cut() {
        let (_d, store) = setup(true);
        let mut s = store.load_session("s").unwrap();
        s.video_anchor_secs = 50.0;
        store.save_session(&s).unwrap();

        let det = detector(&[200.0], false);
        let clips = generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        let ult: Vec<_> = clips
            .iter()
            .filter(|c| c.theme == lgr_core::events::Theme::Ultimate)
            .collect();
        assert_eq!(ult.len(), 1);
        assert!(
            (ult[0].game_time_secs - 250.0).abs() < 1e-9,
            "video 200 s + anchor 50 s"
        );
        assert!(ult[0].file.as_ref().is_some_and(|f| Path::new(f).is_file()));
        let rec = store.load_ultimates("s").unwrap().unwrap();
        assert_eq!((rec.recognized, rec.hud_scale), (true, Some(1.0)));
        assert_eq!(rec.casts, [250.0]);
    }

    #[test]
    fn an_unrecognised_interface_is_cached_reported_and_produces_no_ultimate_clip() {
        let (_d, store) = setup(true);
        let det = unrecognizing();
        let clips = generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert!(clips
            .iter()
            .all(|c| c.theme != lgr_core::events::Theme::Ultimate));
        assert_eq!(clips.len(), 2, "event clips are unaffected");
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 1, "the negative result is cached too");

        let status = ultimate_status(&store, "s").unwrap();
        assert_eq!(status.state, UltimateState::Unrecognized);
        assert_eq!(status.casts, 0);
    }

    #[test]
    fn the_ultimate_status_reflects_the_analysis() {
        let (_d, store) = setup(true);
        assert_eq!(
            ultimate_status(&store, "s").unwrap().state,
            UltimateState::NotAnalyzed
        );
        generate_clips_with(
            &store,
            &cutter(false),
            &detector(&[10.0, 120.0], false),
            "s",
            |_, _| {},
        )
        .unwrap();
        let status = ultimate_status(&store, "s").unwrap();
        assert_eq!(
            (status.state, status.casts, status.hud_scale),
            (UltimateState::Ok, 2, Some(1.0))
        );
    }

    #[test]
    fn a_detection_result_is_cached_and_not_recomputed() {
        let (_d, store) = setup(true);
        let det = detector(&[10.0], false);
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 1);
    }

    #[test]
    fn a_failed_detection_keeps_other_clips_and_is_retried_later() {
        let (_d, store) = setup(true);
        let failing = detector(&[], true);
        let clips = generate_clips_with(&store, &cutter(false), &failing, "s", |_, _| {}).unwrap();
        assert_eq!(clips.len(), 2, "kill clips are unaffected");
        assert_eq!(store.load_ultimates("s").unwrap(), None);

        let working = detector(&[120.0], false);
        generate_clips_with(&store, &cutter(false), &working, "s", |_, _| {}).unwrap();
        assert_eq!(working.calls.get(), 1);
    }

    #[test]
    fn load_clips_includes_cached_ultimates_with_their_files() {
        let (_d, store) = setup(true);
        generate_clips_with(
            &store,
            &cutter(false),
            &detector(&[10.0], false),
            "s",
            |_, _| {},
        )
        .unwrap();
        let clips = load_clips(&store, "s").unwrap();
        let ult = clips
            .iter()
            .find(|c| c.theme == lgr_core::events::Theme::Ultimate)
            .unwrap();
        assert!(ult.file.is_some());
    }

    #[test]
    fn unknown_session_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let err = generate_clips(&store, &cutter(false), "nope", |_, _| {}).unwrap_err();
        assert!(matches!(err, ClipError::Store(StoreError::NotFound)));
    }

    /// Gives the local player of the test session the two given summoner spells.
    fn give_spells(store: &SessionStore, keys: [&str; 2]) {
        let mut s = store.load_session("s").unwrap();
        s.snapshots = vec![lgr_core::session::Snapshot {
            game_time_secs: 0.0,
            players: vec![lgr_core::session::PlayerSnapshot {
                name: "Me#EUW".into(),
                champion: "Ahri".into(),
                team: "ORDER".into(),
                is_me: true,
                level: 1,
                kills: 0,
                deaths: 0,
                assists: 0,
                items: vec![],
                spells: vec![],
                spell_keys: keys.map(str::to_owned).to_vec(),
            }],
            my_gold: None,
        }];
        store.save_session(&s).unwrap();
    }

    fn flash_clips(clips: &[Clip]) -> Vec<&Clip> {
        clips
            .iter()
            .filter(|c| c.theme == lgr_core::events::Theme::Flash)
            .collect()
    }

    #[test]
    fn flash_casts_become_flash_clips_in_game_time() {
        let (_d, store) = setup(true);
        give_spells(&store, ["SummonerDot", "SummonerFlash"]);
        let mut s = store.load_session("s").unwrap();
        s.video_anchor_secs = 10.0;
        store.save_session(&s).unwrap();

        let det = detector_with_summoners(&[], [vec![999.0], vec![60.0, 400.0]], false);
        let clips = generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        let flash = flash_clips(&clips);
        assert_eq!(flash.len(), 2, "the casts of the slot holding Flash only");
        assert!(
            (flash[0].game_time_secs - 70.0).abs() < 1e-9,
            "video 60 s + anchor 10 s"
        );
        assert!(flash[0]
            .file
            .as_ref()
            .is_some_and(|f| Path::new(f).is_file()));
        let record = store.load_ultimates("s").unwrap().unwrap();
        assert_eq!(
            record.summoner_casts,
            Some([vec![1009.0], vec![70.0, 410.0]])
        );
    }

    #[test]
    fn no_flash_clip_when_the_player_has_no_flash() {
        let (_d, store) = setup(true);
        give_spells(&store, ["SummonerHeal", "SummonerDot"]);
        let det = detector_with_summoners(&[], [vec![60.0], vec![400.0]], false);
        let clips = generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert!(flash_clips(&clips).is_empty());
    }

    #[test]
    fn an_old_analysis_without_summoner_spells_is_redone_and_changed_ultimates_are_cut_again() {
        let (_d, store) = setup(true);
        give_spells(&store, ["SummonerFlash", "SummonerDot"]);
        store
            .save_ultimates(
                "s",
                &UltimateRecord {
                    recognized: true,
                    hud_scale: Some(1.1),
                    casts: vec![150.0],
                    summoner_casts: None,
                    analysis_version: 0,
                },
            )
            .unwrap();
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let stale = store.clips_dir("s").unwrap().join("ultimate-0.mp4");
        std::fs::write(&stale, b"old").unwrap();

        let det = detector_with_summoners(&[300.0], [vec![80.0], vec![]], false);
        let c = cutter(false);
        let clips = generate_clips_with(&store, &c, &det, "s", |_, _| {}).unwrap();

        assert_eq!(det.calls.get(), 1);
        let record = store.load_ultimates("s").unwrap().unwrap();
        assert_eq!(record.casts, [300.0]);
        assert!(record.summoner_casts.is_some());
        assert_eq!(flash_clips(&clips).len(), 1);
        assert_ne!(
            std::fs::read(&stale).unwrap(),
            b"old",
            "the ultimate clip was cut again"
        );
    }

    #[test]
    fn an_old_analysis_with_the_same_ultimates_keeps_their_clip_files() {
        let (_d, store) = setup(true);
        store
            .save_ultimates(
                "s",
                &UltimateRecord {
                    recognized: true,
                    hud_scale: Some(1.0),
                    casts: vec![150.0],
                    summoner_casts: None,
                    analysis_version: 0,
                },
            )
            .unwrap();
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let kept = store.clips_dir("s").unwrap().join("ultimate-0.mp4");
        std::fs::write(&kept, b"old").unwrap();

        let det = detector(&[150.0], false);
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(std::fs::read(&kept).unwrap(), b"old");
    }

    #[test]
    fn an_analysis_from_an_older_detection_is_redone_even_with_summoner_casts() {
        // saved by v0.1.8 / v0.1.9: it has summoner spells but the loose ultimate detection
        let (_d, store) = setup(true);
        store
            .save_ultimates(
                "s",
                &UltimateRecord {
                    recognized: true,
                    hud_scale: Some(1.0),
                    casts: vec![150.0, 200.0],
                    summoner_casts: Some([vec![], vec![]]),
                    analysis_version: 0,
                },
            )
            .unwrap();
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        let det = detector(&[300.0], false);
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 1);
        let record = store.load_ultimates("s").unwrap().unwrap();
        assert_eq!(record.casts, [300.0]);
        assert_eq!(record.analysis_version, ANALYSIS_VERSION);
    }

    #[test]
    fn a_current_analysis_is_not_redone() {
        let (_d, store) = setup(true);
        let det = detector(&[300.0], false);
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 1);
    }

    #[test]
    fn an_old_unrecognised_result_is_analysed_again_with_the_current_calibration() {
        let (_d, store) = setup(true);
        store
            .save_ultimates(
                "s",
                &UltimateRecord {
                    recognized: false,
                    hud_scale: None,
                    casts: vec![],
                    summoner_casts: None,
                    analysis_version: 0,
                },
            )
            .unwrap();
        let det = detector(&[120.0], false);
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 1);
        assert_eq!(
            ultimate_status(&store, "s").unwrap().state,
            UltimateState::Ok
        );
    }

    #[test]
    fn an_old_analysis_is_kept_when_the_video_is_gone_or_the_analysis_fails() {
        let (_d, store) = setup(false);
        let old = UltimateRecord {
            recognized: true,
            hud_scale: Some(1.0),
            casts: vec![150.0],
            summoner_casts: None,
            analysis_version: 0,
        };
        store.save_ultimates("s", &old).unwrap();
        let det = detector(&[999.0], false);
        let casts = hud_casts(&store, &det, &store.load_session("s").unwrap());
        assert_eq!(casts.ultimate, [150.0]);
        assert_eq!(det.calls.get(), 0, "no video to read");

        std::fs::write(store.recording_path("s").unwrap(), b"mkv").unwrap();
        let failing = detector(&[], true);
        let casts = hud_casts(&store, &failing, &store.load_session("s").unwrap());
        assert_eq!(casts.ultimate, [150.0]);
        assert_eq!(store.load_ultimates("s").unwrap(), Some(old));
    }

    struct LoadingDetector {
        end: Result<Option<f64>, String>,
        calls: std::cell::Cell<u32>,
    }

    impl UltimateDetector for LoadingDetector {
        fn analyse(&self, _: &Path) -> Result<UltimateOutcome, String> {
            Ok(UltimateOutcome::Unrecognized)
        }

        fn loading_end_secs(&self, _: &Path) -> Result<Option<f64>, String> {
            self.calls.set(self.calls.get() + 1);
            self.end.clone()
        }
    }

    fn loading(end: Result<Option<f64>, String>) -> LoadingDetector {
        LoadingDetector {
            end,
            calls: std::cell::Cell::new(0),
        }
    }

    #[test]
    fn an_anchor_taken_from_a_frozen_loading_clock_is_corrected_and_clips_are_redone() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();
        store
            .save_ultimates(
                "s",
                &UltimateRecord {
                    recognized: true,
                    hud_scale: Some(1.0),
                    casts: vec![150.0],
                    summoner_casts: None,
                    analysis_version: 0,
                },
            )
            .unwrap();

        let det = loading(Ok(Some(70.0)));
        let c = cutter(false);
        let clips = generate_clips_with(&store, &c, &det, "s", |_, _| {}).unwrap();

        let session = store.load_session("s").unwrap();
        assert!((session.video_anchor_secs - -69.5).abs() < 1e-9);
        assert!(session.anchor_verified);
        // 100 s game time - 8 s pre-roll + 69.5 s of loading screen
        assert!((clips[0].start_secs - 161.5).abs() < 1e-9);
        assert_eq!(
            c.calls.borrow().len(),
            clips.len(),
            "every clip is cut again"
        );
        assert!(store
            .load_ultimates("s")
            .unwrap()
            .is_some_and(|r| !r.recognized));
    }

    #[test]
    fn a_recording_without_loading_screen_keeps_its_anchor_and_clips() {
        let (_d, store) = setup(true);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).unwrap();

        let c = cutter(false);
        generate_clips_with(&store, &c, &loading(Ok(None)), "s", |_, _| {}).unwrap();

        let session = store.load_session("s").unwrap();
        assert!(session.video_anchor_secs.abs() < 1e-9 && session.anchor_verified);
        assert!(c.calls.borrow().is_empty());
    }

    #[test]
    fn an_analysis_failure_leaves_the_session_to_be_checked_later() {
        let (_d, store) = setup(true);
        let det = loading(Err("no ffmpeg".into()));
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert!(!store.load_session("s").unwrap().anchor_verified);
    }

    #[test]
    fn a_mid_game_anchor_or_a_verified_session_is_not_analysed() {
        let (_d, store) = setup(true);
        let mut s = store.load_session("s").unwrap();
        s.video_anchor_secs = 251.0;
        store.save_session(&s).unwrap();
        let det = loading(Ok(Some(70.0)));
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        s.video_anchor_secs = 0.0;
        s.anchor_verified = true;
        store.save_session(&s).unwrap();
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).unwrap();
        assert_eq!(det.calls.get(), 0);
        assert!(store.load_session("s").unwrap().video_anchor_secs.abs() < 1e-9);
    }

    #[test]
    fn without_a_recording_the_anchor_cannot_be_checked() {
        let (_d, store) = setup(false);
        generate_clips(&store, &cutter(false), "s", |_, _| {}).ok();
        let det = loading(Ok(Some(70.0)));
        generate_clips_with(&store, &cutter(false), &det, "s", |_, _| {}).ok();
        assert_eq!(det.calls.get(), 0);
    }
}
