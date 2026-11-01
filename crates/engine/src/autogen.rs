//! Generating the clips of each game as soon as it ends.
//!
//! The decision of *which* game to generate lives here, away from the threads and the
//! window, so that it can be tested: [`AutoGenerator`] picks the games that end while
//! the application runs, and [`InFlight`] makes sure a game is never generated twice at
//! the same time (by hand and automatically, for instance).

use lgr_core::store::SessionStore;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Picks, one at a time, the games that ended since it was created.
#[derive(Debug, Default)]
pub struct AutoGenerator {
    seen: HashSet<String>,
}

impl AutoGenerator {
    /// Games already stored are left alone: only the games that end from now on are
    /// generated, so that starting the application never triggers a long backlog.
    #[must_use]
    pub fn new(store: &SessionStore) -> Self {
        let seen = store
            .list()
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.id)
            .collect();
        Self { seen }
    }

    /// The oldest game that ended since the last call, if its video exists and its clips
    /// are not generated yet. A game is looked at once: a failed generation is not retried
    /// in a loop (the user can still generate by hand, and a game is not retried at the
    /// next launch either, since it is then a stored game).
    pub fn next(&mut self, store: &SessionStore) -> Option<String> {
        let mut sessions = store.list().ok()?;
        sessions.reverse(); // `list` is most recent first
        for meta in sessions {
            if !self.seen.insert(meta.id.clone()) {
                continue;
            }
            if meta.has_recording && !meta.clips_ready {
                return Some(meta.id);
            }
        }
        None
    }
}

/// The games being generated right now.
#[derive(Debug, Clone, Default)]
pub struct InFlight(Arc<Mutex<HashSet<String>>>);

/// Held while a game is generated; dropping it frees the game.
#[derive(Debug)]
pub struct InFlightToken {
    set: InFlight,
    id: String,
}

impl InFlight {
    /// Claims `id`, or `None` when it is already being generated.
    #[must_use]
    pub fn acquire(&self, id: &str) -> Option<InFlightToken> {
        let mut set = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        set.insert(id.to_owned()).then(|| InFlightToken {
            set: self.clone(),
            id: id.to_owned(),
        })
    }

    /// Whether `id` is being generated.
    #[must_use]
    pub fn contains(&self, id: &str) -> bool {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(id)
    }
}

impl Drop for InFlightToken {
    fn drop(&mut self) {
        self.set
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lgr_core::session::Session;

    fn add(store: &SessionStore, id: &str, started: u64, recording: bool, clips: bool) {
        store
            .save_session(&Session {
                id: id.into(),
                started_at_ms: started,
                player_aliases: vec![],
                video_anchor_secs: 0.0,
                anchor_verified: false,
                recording_file: None,
                last_game_time_secs: 600.0,
                events: vec![],
                snapshots: vec![],
            })
            .unwrap();
        if recording {
            std::fs::write(store.recording_path(id).unwrap(), b"mkv").unwrap();
        }
        if clips {
            store.save_clips(id, &[clip_stub()]).unwrap();
        }
    }

    fn clip_stub() -> lgr_core::session::Clip {
        lgr_core::session::Clip {
            id: "kill-0".into(),
            theme: lgr_core::events::Theme::Kill,
            label: "Kill".into(),
            game_time_secs: 1.0,
            start_secs: 0.0,
            end_secs: 1.0,
            file: None,
            players: vec![],
            involved: vec![],
            my_gold: None,
            themes: vec![],
            parts: vec![],
        }
    }

    fn store() -> (tempfile::TempDir, SessionStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        (dir, store)
    }

    #[test]
    fn games_already_stored_when_the_application_starts_are_left_alone() {
        let (_d, store) = store();
        add(&store, "g1", 1, true, false);
        let mut auto = AutoGenerator::new(&store);
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn a_game_that_ends_is_picked_once() {
        let (_d, store) = store();
        add(&store, "g1", 1, true, false);
        let mut auto = AutoGenerator::new(&store);
        add(&store, "g2", 2, true, false);
        assert_eq!(auto.next(&store).as_deref(), Some("g2"));
        assert_eq!(
            auto.next(&store),
            None,
            "not picked again, even if it failed"
        );
    }

    #[test]
    fn games_are_picked_oldest_first_one_per_call() {
        let (_d, store) = store();
        let mut auto = AutoGenerator::new(&store);
        add(&store, "g3", 30, true, false);
        add(&store, "g2", 20, true, false);
        assert_eq!(auto.next(&store).as_deref(), Some("g2"));
        assert_eq!(auto.next(&store).as_deref(), Some("g3"));
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn a_game_without_video_or_with_clips_is_skipped_for_good() {
        let (_d, store) = store();
        let mut auto = AutoGenerator::new(&store);
        add(&store, "nov", 1, false, false);
        add(&store, "done", 2, true, true);
        assert_eq!(auto.next(&store), None);
        // the video appearing later does not make it a new game
        std::fs::write(store.recording_path("nov").unwrap(), b"mkv").unwrap();
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn an_empty_or_unreadable_store_gives_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("missing"));
        let mut auto = AutoGenerator::new(&store);
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn a_game_is_never_generated_twice_at_the_same_time() {
        let flight = InFlight::default();
        let first = flight.acquire("g1").expect("free");
        assert!(flight.contains("g1"));
        assert!(flight.acquire("g1").is_none(), "already running");
        assert!(
            flight.acquire("g2").is_some(),
            "another game is independent"
        );
        drop(first);
        assert!(!flight.contains("g1"));
        assert!(flight.acquire("g1").is_some(), "free again once done");
    }
}
