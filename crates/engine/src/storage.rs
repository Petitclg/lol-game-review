//! Disk housekeeping: source deletion after generation and the retention cap.

use lgr_core::session::Clip;
use lgr_core::settings::Settings;
use lgr_core::store::{SessionStore, StoreError};
use std::time::Duration;

/// A folder without timeline older than this is leftover from a crash (no game lasts
/// anywhere near a day).
const ORPHAN_MIN_AGE: Duration = Duration::from_secs(24 * 3600);

/// Deletes the source video once every clip has been cut and the ultimate analysis has
/// run (a failed analysis keeps the video so it can be retried). Returns whether the
/// video was deleted.
pub fn finalize_generation(
    store: &SessionStore,
    settings: &Settings,
    session_id: &str,
    clips: &[Clip],
) -> Result<bool, StoreError> {
    if !settings.delete_source_after_clips {
        return Ok(false);
    }
    let all_cut = clips.iter().all(|c| c.file.is_some());
    if !all_cut || store.load_ultimates(session_id)?.is_none() {
        return Ok(false);
    }
    store.delete_recording(session_id)?;
    Ok(true)
}

/// What a retention pass freed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reclaimed {
    pub session_id: String,
    pub what: &'static str,
    pub bytes: u64,
}

/// Brings the disk usage under `cap_bytes` by deleting, oldest game first, its source
/// video and then its clips. Timelines are kept, and the most recent game is never
/// touched. Folders left by a crash are removed first.
pub fn enforce_retention(
    store: &SessionStore,
    cap_bytes: u64,
) -> Result<Vec<Reclaimed>, StoreError> {
    let mut freed = Vec::new();
    for id in store.stale_orphans(ORPHAN_MIN_AGE)? {
        store.delete_session(&id)?;
    }

    let mut total = store.usage()?.total_bytes;
    if total <= cap_bytes {
        return Ok(freed);
    }
    let sessions = store.list()?;
    // `list` is most recent first: skip the newest, then walk from the oldest.
    for meta in sessions.iter().skip(1).rev() {
        if total <= cap_bytes {
            break;
        }
        let recording = store.recording_bytes(&meta.id)?;
        if recording > 0 {
            store.delete_recording(&meta.id)?;
            total = total.saturating_sub(recording);
            freed.push(Reclaimed {
                session_id: meta.id.clone(),
                what: "recording",
                bytes: recording,
            });
        }
        if total <= cap_bytes {
            break;
        }
        let clips = store.clips_bytes(&meta.id)?;
        if clips > 0 {
            store.delete_clips(&meta.id)?;
            total = total.saturating_sub(clips);
            freed.push(Reclaimed {
                session_id: meta.id.clone(),
                what: "clips",
                bytes: clips,
            });
        }
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgr_core::session::Session;
    use lgr_core::store::UltimateRecord;
    use std::fs;

    fn session(id: &str, started: u64) -> Session {
        Session {
            id: id.into(),
            started_at_ms: started,
            player_aliases: vec![],
            video_anchor_secs: 0.0,
            recording_file: None,
            last_game_time_secs: 600.0,
            events: vec![],
            snapshots: vec![],
        }
    }

    /// A game with a recording and one clip of the given sizes.
    fn add_game(store: &SessionStore, id: &str, started: u64, recording: usize, clip: usize) {
        store.save_session(&session(id, started)).unwrap();
        fs::write(store.recording_path(id).unwrap(), vec![0u8; recording]).unwrap();
        fs::write(
            store.clips_dir(id).unwrap().join("kill-0.mp4"),
            vec![0u8; clip],
        )
        .unwrap();
        store.save_clips(id, &[]).unwrap();
    }

    fn clip(file: Option<&str>) -> Clip {
        Clip {
            id: "kill-0".into(),
            theme: lgr_core::events::Theme::Kill,
            label: "Kill".into(),
            game_time_secs: 1.0,
            start_secs: 0.0,
            end_secs: 1.0,
            file: file.map(str::to_owned),
            players: vec![],
            involved: vec![],
            my_gold: None,
        }
    }

    fn analysed(store: &SessionStore, id: &str) {
        store
            .save_ultimates(
                id,
                &UltimateRecord {
                    recognized: true,
                    hud_scale: Some(1.0),
                    casts: vec![],
                },
            )
            .unwrap();
    }

    #[test]
    fn the_source_is_deleted_only_when_everything_is_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, 1000, 100);
        let on = Settings::default();

        // analysis not done yet: keep the video
        assert!(!finalize_generation(&store, &on, "a", &[clip(Some("x"))]).unwrap());
        analysed(&store, "a");
        // a clip failed to cut: keep the video so generation can be retried
        assert!(!finalize_generation(&store, &on, "a", &[clip(Some("x")), clip(None)]).unwrap());
        // the setting is off
        let off = Settings {
            delete_source_after_clips: false,
            ..Settings::default()
        };
        assert!(!finalize_generation(&store, &off, "a", &[clip(Some("x"))]).unwrap());
        assert_eq!(store.recording_bytes("a").unwrap(), 1000);

        assert!(finalize_generation(&store, &on, "a", &[clip(Some("x"))]).unwrap());
        assert_eq!(store.recording_bytes("a").unwrap(), 0);
        assert!(store.clips_bytes("a").unwrap() > 0, "clips are kept");
    }

    #[test]
    fn retention_does_nothing_under_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, 1000, 100);
        assert!(enforce_retention(&store, 1_000_000).unwrap().is_empty());
        assert_eq!(store.recording_bytes("a").unwrap(), 1000);
    }

    #[test]
    fn retention_frees_the_oldest_game_first_recording_before_clips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "old", 1, 10_000, 3_000);
        add_game(&store, "mid", 2, 10_000, 3_000);
        add_game(&store, "new", 3, 10_000, 3_000);
        let total = store.usage().unwrap().total_bytes;

        // just enough to need only the oldest recording to go
        let freed = enforce_retention(&store, total - 5_000).unwrap();
        assert_eq!(
            freed,
            [Reclaimed {
                session_id: "old".into(),
                what: "recording",
                bytes: 10_000
            }]
        );
        assert_eq!(store.recording_bytes("old").unwrap(), 0);
        assert_eq!(store.recording_bytes("mid").unwrap(), 10_000);
        assert!(
            store.clips_bytes("old").unwrap() > 0,
            "clips survive when enough"
        );
    }

    #[test]
    fn retention_goes_on_to_clips_then_to_the_next_game_but_spares_the_newest() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "old", 1, 10_000, 3_000);
        add_game(&store, "new", 3, 10_000, 3_000);

        let freed = enforce_retention(&store, 1).unwrap();
        let what: Vec<_> = freed
            .iter()
            .map(|r| (r.session_id.as_str(), r.what))
            .collect();
        assert_eq!(what, [("old", "recording"), ("old", "clips")]);
        assert_eq!(
            store.recording_bytes("new").unwrap(),
            10_000,
            "newest game is spared"
        );
        assert!(store.load_session("old").is_ok(), "timelines are kept");
    }

    #[test]
    fn stale_folders_without_a_timeline_are_cleaned_but_fresh_ones_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        fs::create_dir_all(dir.path().join("g-in-progress")).unwrap();
        fs::write(dir.path().join("g-in-progress").join("recording.mkv"), b"x").unwrap();
        // a brand new folder is a game in progress, never an orphan
        assert!(store.stale_orphans(ORPHAN_MIN_AGE).unwrap().is_empty());
        // with a zero age threshold it is reported, proving the detection works
        assert_eq!(
            store.stale_orphans(Duration::ZERO).unwrap(),
            ["g-in-progress"]
        );
    }
}
