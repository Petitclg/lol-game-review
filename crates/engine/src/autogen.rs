//! Generating the clips of each game without the player asking.
//!
//! The decision of *which* game to generate lives here, away from the threads and the
//! window, so that it can be tested: [`AutoGenerator`] picks the games whose clips are still
//! to be generated, and [`InFlight`] makes sure a game is never generated twice at the same
//! time (by hand and automatically, for instance).

use lgr_core::store::SessionStore;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Picks, one at a time, the games whose clips are still to be generated.
#[derive(Debug, Default)]
pub struct AutoGenerator {
    attempted: HashSet<String>,
}

impl AutoGenerator {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The most recent game that is over, whose video exists and whose clips are not
    /// generated yet: the game that just ended, or one that ended while the application was
    /// closed or restarting. A game is picked once per run: a failed generation is not
    /// retried in a loop (the player can still generate by hand, and it is tried again at
    /// the next launch).
    pub fn next(&mut self, store: &SessionStore) -> Option<String> {
        store
            .pending_generation()
            .into_iter()
            .find(|id| self.attempted.insert(id.clone()))
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
    fn a_game_recorded_before_the_application_started_is_picked() {
        // e.g. the game ended while the application was closed or restarting
        let (_d, store) = store();
        add(&store, "g1", 1, true, false);
        let mut auto = AutoGenerator::new();
        assert_eq!(auto.next(&store).as_deref(), Some("g1"));
    }

    #[test]
    fn a_game_is_picked_once_even_if_its_generation_failed() {
        let (_d, store) = store();
        add(&store, "g1", 1, true, false);
        let mut auto = AutoGenerator::new();
        assert_eq!(auto.next(&store).as_deref(), Some("g1"));
        assert_eq!(auto.next(&store), None, "not picked again in the same run");
        assert_eq!(
            AutoGenerator::new().next(&store).as_deref(),
            Some("g1"),
            "but tried again at the next launch"
        );
    }

    #[test]
    fn the_most_recent_game_comes_first_one_per_call() {
        let (_d, store) = store();
        add(&store, "g20", 20, true, false);
        add(&store, "g30", 30, true, false);
        add(&store, "g10", 10, true, false);
        let mut auto = AutoGenerator::new();
        assert_eq!(auto.next(&store).as_deref(), Some("g30"));
        assert_eq!(auto.next(&store).as_deref(), Some("g20"));
        assert_eq!(auto.next(&store).as_deref(), Some("g10"));
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn a_game_that_ends_later_is_picked_too() {
        let (_d, store) = store();
        let mut auto = AutoGenerator::new();
        assert_eq!(auto.next(&store), None);
        add(&store, "g5", 5, true, false);
        assert_eq!(auto.next(&store).as_deref(), Some("g5"));
    }

    #[test]
    fn a_game_without_video_or_with_clips_is_never_picked() {
        let (_d, store) = store();
        let mut auto = AutoGenerator::new();
        add(&store, "g1", 1, false, false);
        add(&store, "g2", 2, true, true);
        assert_eq!(auto.next(&store), None);
    }

    #[test]
    fn an_empty_or_unreadable_store_gives_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("missing"));
        assert_eq!(AutoGenerator::new().next(&store), None);
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
