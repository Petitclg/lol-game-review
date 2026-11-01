//! Disk housekeeping: source deletion after generation and the retention cap.

use lgr_core::session::Clip;
use lgr_core::settings::{Settings, BYTES_PER_GB};
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
    remove_orphans(store)?;
    let total = store.usage()?.total_bytes;
    Ok(free_oldest(store, total.saturating_sub(cap_bytes))?.0)
}

fn remove_orphans(store: &SessionStore) -> Result<(), StoreError> {
    for id in store.stale_orphans(ORPHAN_MIN_AGE)? {
        store.delete_session(&id)?;
    }
    Ok(())
}

/// Deletes, oldest game first, source videos and then clips until `needed` bytes are
/// freed. The most recent game is never touched. Returns what was deleted and the bytes
/// freed.
fn free_oldest(store: &SessionStore, needed: u64) -> Result<(Vec<Reclaimed>, u64), StoreError> {
    let mut freed = Vec::new();
    let mut total_freed = 0u64;
    if needed == 0 {
        return Ok((freed, 0));
    }
    let sessions = store.list()?;
    // `list` is most recent first: skip the newest, then walk from the oldest.
    for meta in sessions.iter().skip(1).rev() {
        if total_freed >= needed {
            break;
        }
        let recording = store.recording_bytes(&meta.id)?;
        if recording > 0 {
            store.delete_recording(&meta.id)?;
            total_freed += recording;
            freed.push(Reclaimed {
                session_id: meta.id.clone(),
                what: "recording",
                bytes: recording,
            });
        }
        if total_freed >= needed {
            break;
        }
        let clips = store.clips_bytes(&meta.id)?;
        if clips > 0 {
            store.delete_clips(&meta.id)?;
            total_freed += clips;
            freed.push(Reclaimed {
                session_id: meta.id.clone(),
                what: "clips",
                bytes: clips,
            });
        }
    }
    Ok((freed, total_freed))
}

/// Space kept free for a recording: at about 5 Mbit/s, 4 GiB covers a game of more than
/// an hour and a half.
pub const RECORDING_RESERVE_BYTES: u64 = 4 * BYTES_PER_GB;

/// What making room before a recording did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomReport {
    pub reclaimed: Vec<Reclaimed>,
    /// Bytes still missing after deleting everything that may be deleted.
    pub short_by: u64,
}

/// Makes room for the recording of a game that is about to start, by deleting the oldest
/// data (source videos, then clips, game after game; timelines and the most recent game
/// are kept).
///
/// Two limits can leave no room: the disk budget of the settings, if any (the recording
/// must fit under it), and the free space of the disk itself, when known
/// (`free_disk_bytes`). Nothing is deleted when the option is off or both have room.
pub fn make_room(
    store: &SessionStore,
    settings: &Settings,
    free_disk_bytes: Option<u64>,
) -> Result<RoomReport, StoreError> {
    make_room_reserving(store, settings, free_disk_bytes, RECORDING_RESERVE_BYTES)
}

fn make_room_reserving(
    store: &SessionStore,
    settings: &Settings,
    free_disk_bytes: Option<u64>,
    reserve: u64,
) -> Result<RoomReport, StoreError> {
    if !settings.make_room_for_recording {
        return Ok(RoomReport {
            reclaimed: Vec::new(),
            short_by: 0,
        });
    }
    remove_orphans(store)?;
    let total = store.usage()?.total_bytes;
    let over_budget = settings
        .retention_bytes()
        .map_or(0, |cap| total.saturating_sub(cap.saturating_sub(reserve)));
    let over_disk = free_disk_bytes.map_or(0, |free| reserve.saturating_sub(free));
    let needed = over_budget.max(over_disk);
    let (reclaimed, freed) = free_oldest(store, needed)?;
    Ok(RoomReport {
        reclaimed,
        short_by: needed.saturating_sub(freed),
    })
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
            anchor_verified: true,
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
                    summoner_casts: Some([vec![], vec![]]),
                    analysis_version: lgr_core::ultimate::ANALYSIS_VERSION,
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

    /// Three games of 13 000 bytes each (10 000 recording + 3 000 clip + timeline).
    fn three_games() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "old", 1, 10_000, 3_000);
        add_game(&store, "mid", 2, 10_000, 3_000);
        add_game(&store, "new", 3, 10_000, 3_000);
        (dir, store)
    }

    fn unlimited() -> Settings {
        Settings {
            retention_gb: None,
            ..Settings::default()
        }
    }

    #[test]
    fn a_full_disk_loses_the_oldest_data_to_make_room_for_the_recording() {
        let (_d, store) = three_games();
        // 4 000 bytes free, 15 000 needed: 11 000 to find
        let report = make_room_reserving(&store, &unlimited(), Some(4_000), 15_000).unwrap();
        let what: Vec<_> = report
            .reclaimed
            .iter()
            .map(|r| (r.session_id.as_str(), r.what))
            .collect();
        assert_eq!(what, [("old", "recording"), ("old", "clips")]);
        assert_eq!(report.short_by, 0);
        assert_eq!(store.recording_bytes("mid").unwrap(), 10_000, "kept");
        assert_eq!(store.recording_bytes("new").unwrap(), 10_000, "kept");
    }

    #[test]
    fn nothing_is_deleted_when_the_disk_has_room() {
        let (_d, store) = three_games();
        let report = make_room_reserving(&store, &unlimited(), Some(20_000), 15_000).unwrap();
        assert!(report.reclaimed.is_empty());
        assert_eq!(report.short_by, 0);
        let unknown = make_room_reserving(&store, &unlimited(), None, 15_000).unwrap();
        assert!(
            unknown.reclaimed.is_empty(),
            "unknown free space, no budget"
        );
    }

    #[test]
    fn the_recording_must_fit_under_the_budget_too() {
        let (_d, store) = three_games();
        let total = store.usage().unwrap().total_bytes;
        let capped = Settings {
            retention_gb: Some(1),
            ..Settings::default()
        };
        // the budget (1 GiB) minus the reserve leaves 5 000 bytes: the games must make way
        let report = make_room_reserving(&store, &capped, None, BYTES_PER_GB - 5_000).unwrap();
        assert!(total > 5_000);
        assert_eq!(
            report
                .reclaimed
                .iter()
                .map(|r| r.session_id.as_str())
                .collect::<Vec<_>>(),
            ["old", "old", "mid", "mid"]
        );
        let freed: u64 = report.reclaimed.iter().map(|r| r.bytes).sum();
        assert_eq!(
            report.short_by,
            total - 5_000 - freed,
            "the newest game stays"
        );
        assert_eq!(store.recording_bytes("new").unwrap(), 10_000);
    }

    #[test]
    fn the_tighter_of_the_two_limits_decides() {
        let (_d, store) = three_games();
        let capped = Settings {
            retention_gb: Some(1),
            ..Settings::default()
        };
        // the budget leaves room (1 GiB - 100 bytes reserved); the disk needs 12 000 more
        let report = make_room_reserving(&store, &capped, Some(3_000), 15_000).unwrap();
        assert_eq!(report.reclaimed.len(), 2, "only the oldest game");
        assert_eq!(report.short_by, 0);
    }

    #[test]
    fn the_newest_game_is_never_deleted_even_when_that_leaves_the_disk_short() {
        let (_d, store) = three_games();
        let report = make_room_reserving(&store, &unlimited(), Some(0), 1_000_000).unwrap();
        assert_eq!(
            report.reclaimed.len(),
            4,
            "old and mid, recording and clips"
        );
        let freed: u64 = report.reclaimed.iter().map(|r| r.bytes).sum();
        assert_eq!(report.short_by, 1_000_000 - freed);
        assert_eq!(store.recording_bytes("new").unwrap(), 10_000);
        assert!(store.load_session("old").is_ok(), "timelines are kept");
    }

    #[test]
    fn the_option_can_be_turned_off() {
        let (_d, store) = three_games();
        let off = Settings {
            make_room_for_recording: false,
            retention_gb: None,
            ..Settings::default()
        };
        let report = make_room_reserving(&store, &off, Some(0), 15_000).unwrap();
        assert!(report.reclaimed.is_empty());
        assert_eq!(report.short_by, 0);
        assert_eq!(store.recording_bytes("old").unwrap(), 10_000);
    }

    #[test]
    fn the_real_reserve_covers_a_long_game() {
        // 5 Mbit/s for 1 h 45 is about 3.7 GiB
        const LONG_GAME_BYTES: u64 = 5_000_000 / 8 * 6_300;
        const { assert!(RECORDING_RESERVE_BYTES > LONG_GAME_BYTES) };
    }
}
