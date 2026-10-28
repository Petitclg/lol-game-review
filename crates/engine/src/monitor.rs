//! Game monitor: detects games through the Live Client API, records them and
//! persists a session when they end. Driven by `tick`, so it is deterministic to test.

use lgr_core::store::SessionStore;
use lgr_live::{AllGameData, LiveSource, SessionTracker};
use serde::Serialize;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameState {
    Idle,
    InGame,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub state: GameState,
    pub recording: bool,
    pub ffmpeg_available: bool,
}

/// Starts recordings; abstracted so the monitor can be tested without ffmpeg.
pub trait RecorderFactory {
    fn available(&self) -> bool;
    fn start(&self, output: &Path) -> Result<Box<dyn ActiveRecording>, String>;
}

pub trait ActiveRecording {
    fn stop(self: Box<Self>);
}

type StatusCallback = Arc<dyn Fn(&Status) + Send + Sync>;

struct Active {
    tracker: SessionTracker,
    recording: Option<Box<dyn ActiveRecording>>,
    misses: u32,
}

/// Consecutive failed polls before a game is considered over (covers short hiccups).
const MAX_MISSES: u32 = 3;
/// Games shorter than this (remakes, aborted loads) are discarded.
const MIN_GAME_SECS: f64 = 60.0;
/// Estimated delay between spawning the recorder and its first frame.
const RECORDER_STARTUP_MS: u64 = 300;
const IDLE_POLL: Duration = Duration::from_secs(5);
const GAME_POLL: Duration = Duration::from_secs(3);

pub struct Monitor<L: LiveSource, R: RecorderFactory> {
    live: L,
    recorder: R,
    store: SessionStore,
    active: Option<Active>,
    on_status: StatusCallback,
}

impl<L: LiveSource, R: RecorderFactory> Monitor<L, R> {
    pub fn new(live: L, recorder: R, store: SessionStore, on_status: StatusCallback) -> Self {
        Self {
            live,
            recorder,
            store,
            active: None,
            on_status,
        }
    }

    #[must_use]
    pub fn status(&self) -> Status {
        Status {
            state: if self.active.is_some() {
                GameState::InGame
            } else {
                GameState::Idle
            },
            recording: self.active.as_ref().is_some_and(|a| a.recording.is_some()),
            ffmpeg_available: self.recorder.available(),
        }
    }

    /// How long to wait before the next `tick`.
    #[must_use]
    pub fn next_interval(&self) -> Duration {
        if self.active.is_some() {
            GAME_POLL
        } else {
            IDLE_POLL
        }
    }

    pub fn tick(&mut self, now_ms: u64) {
        let data = self.live.fetch().ok().flatten();
        match (data, self.active.is_some()) {
            (Some(d), false) => self.begin(&d, now_ms),
            (Some(d), true) => self.observe(&d, now_ms),
            (None, true) => self.miss(),
            (None, false) => {}
        }
    }

    /// Finishes any game in progress (call when the application quits).
    pub fn shutdown(&mut self) {
        if self.active.is_some() {
            self.end();
        }
    }

    fn emit(&self) {
        (self.on_status)(&self.status());
    }

    fn begin(&mut self, data: &AllGameData, now_ms: u64) {
        let id = format!("g{now_ms}");
        if self.store.prepare(&id).is_err() {
            return;
        }
        let recording = self
            .store
            .recording_path(&id)
            .ok()
            .and_then(|path| self.recorder.start(&path).ok());
        let file = recording
            .as_ref()
            .and_then(|_| self.store.recording_path(&id).ok())
            .map(|p| p.to_string_lossy().into_owned());
        let mut tracker = SessionTracker::new(id, now_ms + RECORDER_STARTUP_MS, file);
        tracker.observe(data, now_ms);
        self.active = Some(Active {
            tracker,
            recording,
            misses: 0,
        });
        self.emit();
    }

    fn observe(&mut self, data: &AllGameData, now_ms: u64) {
        if let Some(a) = self.active.as_mut() {
            a.misses = 0;
            a.tracker.observe(data, now_ms);
        }
        if data.has_game_ended() {
            self.end();
        }
    }

    fn miss(&mut self) {
        let over = self.active.as_mut().is_some_and(|a| {
            a.misses += 1;
            a.misses >= MAX_MISSES
        });
        if over {
            self.end();
        }
    }

    fn end(&mut self) {
        let Some(active) = self.active.take() else {
            return;
        };
        if let Some(rec) = active.recording {
            rec.stop();
        }
        let session = active.tracker.finish();
        if session.last_game_time_secs < MIN_GAME_SECS {
            let _ = self.store.delete_session(&session.id);
        } else {
            let _ = self.store.save_session(&session);
        }
        self.emit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgr_live::LiveError;
    use std::cell::Cell;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct FakeLive(Mutex<VecDeque<Option<AllGameData>>>);

    impl LiveSource for FakeLive {
        fn fetch(&self) -> Result<Option<AllGameData>, LiveError> {
            Ok(self.0.lock().unwrap().pop_front().flatten())
        }
    }

    struct FakeRec {
        fail: bool,
        started: Cell<u32>,
        stopped: Arc<Mutex<u32>>,
    }

    struct Handle(Arc<Mutex<u32>>);
    impl ActiveRecording for Handle {
        fn stop(self: Box<Self>) {
            *self.0.lock().unwrap() += 1;
        }
    }

    impl RecorderFactory for FakeRec {
        fn available(&self) -> bool {
            !self.fail
        }
        fn start(&self, output: &Path) -> Result<Box<dyn ActiveRecording>, String> {
            if self.fail {
                return Err("no ffmpeg".into());
            }
            self.started.set(self.started.get() + 1);
            std::fs::write(output, b"mkv").unwrap();
            Ok(Box::new(Handle(self.stopped.clone())))
        }
    }

    fn data(game_time: f64, ended: bool) -> AllGameData {
        let end = if ended {
            r#",{"EventID":9,"EventName":"GameEnd","EventTime":1.0}"#
        } else {
            ""
        };
        let json = format!(
            r#"{{"activePlayer":{{"riotId":"Me#EUW","riotIdGameName":"Me","summonerName":"Me#EUW"}},
            "allPlayers":[],
            "events":{{"Events":[{{"EventID":1,"EventName":"ChampionKill","EventTime":70.0,"KillerName":"Me","VictimName":"X"}}{end}]}},
            "gameData":{{"gameTime":{game_time}}}}}"#
        );
        AllGameData::parse(&json).unwrap()
    }

    struct Harness {
        _dir: tempfile::TempDir,
        store: SessionStore,
        monitor: Monitor<FakeLive, FakeRec>,
        stopped: Arc<Mutex<u32>>,
        statuses: Arc<Mutex<Vec<Status>>>,
    }

    fn harness(script: Vec<Option<AllGameData>>, fail_rec: bool) -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let stopped = Arc::new(Mutex::new(0));
        let statuses = Arc::new(Mutex::new(Vec::new()));
        let sink = statuses.clone();
        let monitor = Monitor::new(
            FakeLive(Mutex::new(script.into())),
            FakeRec {
                fail: fail_rec,
                started: Cell::new(0),
                stopped: stopped.clone(),
            },
            store.clone(),
            Arc::new(move |s| sink.lock().unwrap().push(s.clone())),
        );
        Harness {
            _dir: dir,
            store,
            monitor,
            stopped,
            statuses,
        }
    }

    #[test]
    fn stays_idle_without_a_game() {
        let mut h = harness(vec![None, None], false);
        h.monitor.tick(1_000);
        h.monitor.tick(6_000);
        assert_eq!(h.monitor.status().state, GameState::Idle);
        assert!(h.store.list().unwrap().is_empty());
        assert!(h.statuses.lock().unwrap().is_empty());
        assert_eq!(h.monitor.next_interval(), IDLE_POLL);
    }

    #[test]
    fn records_a_game_and_saves_the_session_on_game_end() {
        let mut h = harness(
            vec![
                Some(data(5.0, false)),
                Some(data(120.0, false)),
                Some(data(1800.0, true)),
            ],
            false,
        );
        h.monitor.tick(10_000);
        let s = h.monitor.status();
        assert_eq!((s.state, s.recording), (GameState::InGame, true));
        assert_eq!(h.monitor.next_interval(), GAME_POLL);
        h.monitor.tick(13_000);
        h.monitor.tick(1_810_000);

        assert_eq!(h.monitor.status().state, GameState::Idle);
        assert_eq!(*h.stopped.lock().unwrap(), 1);
        let metas = h.store.list().unwrap();
        assert_eq!(metas.len(), 1);
        assert!(metas[0].has_recording);
        let session = h.store.load_session(&metas[0].id).unwrap();
        assert!(session.recording_file.is_some());
        assert_eq!(session.events.len(), 2);
        assert!(session.last_game_time_secs >= 1800.0);
        assert_eq!(h.statuses.lock().unwrap().len(), 2);
    }

    #[test]
    fn game_ends_after_consecutive_misses_only() {
        let mut h = harness(
            vec![
                Some(data(100.0, false)),
                None,
                None,
                Some(data(110.0, false)),
                None,
                None,
                None,
            ],
            false,
        );
        for t in 0..6 {
            h.monitor.tick(t * 3_000);
        }
        assert_eq!(
            h.monitor.status().state,
            GameState::InGame,
            "hiccup tolerated"
        );
        h.monitor.tick(18_000);
        assert_eq!(h.monitor.status().state, GameState::Idle);
        assert_eq!(h.store.list().unwrap().len(), 1);
    }

    #[test]
    fn short_games_are_discarded() {
        let mut h = harness(vec![Some(data(20.0, false)), Some(data(30.0, true))], false);
        h.monitor.tick(1_000);
        h.monitor.tick(4_000);
        assert!(h.store.list().unwrap().is_empty());
        assert_eq!(*h.stopped.lock().unwrap(), 1);
    }

    #[test]
    fn timeline_is_kept_when_recording_is_unavailable() {
        let mut h = harness(
            vec![Some(data(100.0, false)), Some(data(200.0, true))],
            true,
        );
        h.monitor.tick(1_000);
        assert!(!h.monitor.status().recording);
        assert!(!h.monitor.status().ffmpeg_available);
        h.monitor.tick(4_000);
        let metas = h.store.list().unwrap();
        assert_eq!(metas.len(), 1);
        assert!(!metas[0].has_recording);
        assert!(h
            .store
            .load_session(&metas[0].id)
            .unwrap()
            .recording_file
            .is_none());
    }

    #[test]
    fn shutdown_finalizes_a_running_game() {
        let mut h = harness(vec![Some(data(300.0, false))], false);
        h.monitor.tick(1_000);
        h.monitor.shutdown();
        assert_eq!(*h.stopped.lock().unwrap(), 1);
        assert_eq!(h.store.list().unwrap().len(), 1);
        h.monitor.shutdown();
        assert_eq!(*h.stopped.lock().unwrap(), 1);
    }
}
