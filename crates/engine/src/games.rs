//! Merges the LCU match history with locally recorded sessions.

use lgr_core::store::{GameSummary, SessionMeta, SessionStore};
use lgr_lcu::MatchSummary;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEntry {
    pub game_id: u64,
    pub created_ms: u64,
    pub duration_secs: u32,
    /// 0 when the game is only known from its recording.
    pub queue_id: u32,
    /// 0 when the game is only known from its recording: see `champion`.
    pub champion_id: u32,
    /// Data Dragon id of the champion, known for recorded games only.
    pub champion: Option<String>,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    /// `None` when the game is only known from its recording (the outcome comes from the
    /// client).
    pub win: Option<bool>,
    /// Riot ID of the account that played.
    pub account: Option<String>,
    pub session_id: Option<String>,
    pub clips_ready: bool,
    /// The source video still exists, so clips can be (re)generated.
    pub has_recording: bool,
}

impl GameEntry {
    /// What is kept with the game once it has left the history of the client; `None`
    /// while the client has not said how the game ended.
    #[must_use]
    pub fn summary(&self) -> Option<GameSummary> {
        Some(GameSummary {
            game_id: self.game_id,
            created_ms: self.created_ms,
            duration_secs: self.duration_secs,
            queue_id: self.queue_id,
            champion_id: self.champion_id,
            kills: self.kills,
            deaths: self.deaths,
            assists: self.assists,
            win: self.win?,
        })
    }
}

/// Saves the summary of every listed game that has a recorded session. Best effort: a
/// game that cannot be written is simply not remembered.
pub fn remember_games(store: &SessionStore, games: &[GameEntry]) {
    for game in games {
        if let (Some(id), Some(summary)) = (&game.session_id, game.summary()) {
            let _ = store.save_game_summary(id, &summary);
        }
    }
}

/// Every game that can be shown: the games of the client history (`account` being the
/// connected account; no matches when the client is off) and every other recorded game,
/// which needs no client. Most recent first. The summaries of the paired games are saved.
pub fn list_games(
    store: &SessionStore,
    sessions: &[SessionMeta],
    matches: &[MatchSummary],
    account: Option<&str>,
) -> Vec<GameEntry> {
    let mut games = merge_games(matches, sessions, account);
    remember_games(store, &games);
    let paired: HashSet<String> = games.iter().filter_map(|g| g.session_id.clone()).collect();
    for session in sessions {
        if (session.has_recording || session.clips_ready) && !paired.contains(&session.id) {
            let summary = store.load_game_summary(&session.id).unwrap_or_default();
            games.push(recorded_game(session, summary.as_ref()));
        }
    }
    games.sort_by_key(|g| std::cmp::Reverse(g.created_ms));
    games
}

/// A recorded game that is not in the client history: what was kept of the history when
/// it was there, else what the recording itself says.
fn recorded_game(session: &SessionMeta, summary: Option<&GameSummary>) -> GameEntry {
    let (game_id, created_ms, duration_secs, queue_id, champion_id, win) = match summary {
        Some(s) => (
            s.game_id,
            s.created_ms,
            s.duration_secs,
            s.queue_id,
            s.champion_id,
            Some(s.win),
        ),
        None => (
            session.started_at_ms,
            session.started_at_ms,
            session.last_game_time_secs as u32,
            0,
            0,
            None,
        ),
    };
    GameEntry {
        game_id,
        created_ms,
        duration_secs,
        queue_id,
        champion_id,
        champion: session.champion.clone(),
        kills: summary.map_or(session.kills, |s| s.kills),
        deaths: summary.map_or(session.deaths, |s| s.deaths),
        assists: summary.map_or(session.assists, |s| s.assists),
        win,
        account: session.account.clone(),
        session_id: Some(session.id.clone()),
        clips_ready: session.clips_ready,
        has_recording: session.has_recording,
    }
}

/// A recording starts when the live API becomes available, i.e. after loading,
/// so it begins shortly after the game was created.
const MAX_LEAD_MS: i64 = 15 * 60 * 1000;
const MAX_LAG_MS: i64 = 60 * 1000;

/// Pairs games with recorded sessions started within the expected window after the
/// game's creation. Closest pairs are assigned first and a session is used once. The
/// games belong to `account`, the connected account (else to the one that recorded them).
#[must_use]
fn merge_games(
    matches: &[MatchSummary],
    sessions: &[SessionMeta],
    account: Option<&str>,
) -> Vec<GameEntry> {
    let usable: Vec<&SessionMeta> = sessions
        .iter()
        .filter(|s| s.has_recording || s.clips_ready)
        .collect();

    let mut candidates: Vec<(i64, usize, usize)> = Vec::new();
    for (mi, m) in matches.iter().enumerate() {
        for (si, s) in usable.iter().enumerate() {
            let delta = s.started_at_ms as i64 - m.created_ms as i64;
            if (-MAX_LAG_MS..=MAX_LEAD_MS).contains(&delta) {
                candidates.push((delta.abs(), mi, si));
            }
        }
    }
    candidates.sort_unstable();

    let mut assigned: Vec<Option<usize>> = vec![None; matches.len()];
    let mut used = vec![false; usable.len()];
    for (_, mi, si) in candidates {
        if assigned[mi].is_none() && !used[si] {
            assigned[mi] = Some(si);
            used[si] = true;
        }
    }

    matches
        .iter()
        .zip(assigned)
        .map(|(m, si)| {
            let session = si.map(|i| usable[i]);
            GameEntry {
                game_id: m.game_id,
                created_ms: m.created_ms,
                duration_secs: m.duration_secs,
                queue_id: m.queue_id,
                champion_id: m.champion_id,
                champion: session.and_then(|s| s.champion.clone()),
                kills: m.kills,
                deaths: m.deaths,
                assists: m.assists,
                win: Some(m.win),
                account: account
                    .map(str::to_owned)
                    .or_else(|| session.and_then(|s| s.account.clone())),
                session_id: session.map(|s| s.id.clone()),
                clips_ready: session.is_some_and(|s| s.clips_ready),
                has_recording: session.is_some_and(|s| s.has_recording),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game(id: u64, created: u64) -> MatchSummary {
        MatchSummary {
            game_id: id,
            created_ms: created,
            duration_secs: 1800,
            queue_id: 420,
            champion_id: 1,
            kills: 1,
            deaths: 2,
            assists: 3,
            win: true,
        }
    }

    fn session(id: &str, started: u64, rec: bool, clips: bool) -> SessionMeta {
        SessionMeta {
            id: id.into(),
            started_at_ms: started,
            last_game_time_secs: 1800.0,
            has_recording: rec,
            clips_ready: clips,
            account: Some("Me#EUW".into()),
            champion: Some("Ahri".into()),
            kills: 4,
            deaths: 5,
            assists: 6,
        }
    }

    const MIN: u64 = 60_000;

    fn merged(matches: &[MatchSummary], sessions: &[SessionMeta]) -> Vec<GameEntry> {
        merge_games(matches, sessions, None)
    }

    #[test]
    fn pairs_session_started_shortly_after_creation() {
        let g = merged(&[game(1, 10 * MIN)], &[session("a", 12 * MIN, true, false)]);
        assert_eq!(g[0].session_id.as_deref(), Some("a"));
        assert!(!g[0].clips_ready);
    }

    #[test]
    fn ignores_sessions_outside_the_window() {
        let far = merged(&[game(1, 10 * MIN)], &[session("a", 40 * MIN, true, false)]);
        let before = merged(&[game(1, 10 * MIN)], &[session("a", 5 * MIN, true, false)]);
        assert!(far[0].session_id.is_none() && before[0].session_id.is_none());
    }

    #[test]
    fn sessions_without_video_or_clips_are_not_offered() {
        let g = merged(
            &[game(1, 10 * MIN)],
            &[session("a", 11 * MIN, false, false)],
        );
        assert!(g[0].session_id.is_none());
    }

    #[test]
    fn clips_only_session_stays_available() {
        let g = merged(&[game(1, 10 * MIN)], &[session("a", 11 * MIN, false, true)]);
        assert!(g[0].clips_ready);
        assert!(!g[0].has_recording, "the source video was deleted");
    }

    #[test]
    fn a_session_with_its_video_reports_it() {
        let g = merged(&[game(1, 10 * MIN)], &[session("a", 11 * MIN, true, true)]);
        assert!(g[0].has_recording && g[0].clips_ready);
    }

    #[test]
    fn the_connected_account_owns_the_games_of_its_history() {
        let sessions = [session("a", 12 * MIN, true, false)];
        let matches = [game(1, 10 * MIN), game(2, 500 * MIN)];
        let g = merge_games(&matches, &sessions, Some("Other#EUW"));
        assert_eq!(g[0].account.as_deref(), Some("Other#EUW"));
        assert_eq!(g[1].account.as_deref(), Some("Other#EUW"));
        let g = merged(&matches, &sessions);
        assert_eq!(
            g[0].account.as_deref(),
            Some("Me#EUW"),
            "the recording knows it"
        );
        assert_eq!(g[1].account, None);
    }

    fn saved_session(store: &SessionStore, id: &str, started: u64) {
        store
            .save_session(&lgr_core::session::Session {
                id: id.into(),
                started_at_ms: started,
                player_aliases: vec!["Me#EUW".into()],
                video_anchor_secs: 0.0,
                anchor_verified: false,
                recording_file: None,
                last_game_time_secs: 1800.0,
                events: vec![],
                snapshots: vec![],
            })
            .unwrap();
        std::fs::write(store.recording_path(id).unwrap(), b"mkv").unwrap();
    }

    #[test]
    fn the_summary_of_a_listed_game_is_saved_with_its_session_only() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        saved_session(&store, "a", 12 * MIN);
        let sessions = store.list().unwrap();
        let games = merged(&[game(1, 10 * MIN), game(2, 500 * MIN)], &sessions);
        remember_games(&store, &games);
        let kept = store.load_game_summary("a").unwrap().expect("saved");
        assert_eq!((kept.game_id, kept.champion_id, kept.win), (1, 1, true));
        assert_eq!(Some(kept), games[0].summary());
        assert_eq!(
            store.list().unwrap().len(),
            1,
            "no folder for a game without a session"
        );
    }

    #[test]
    fn recorded_games_are_listed_without_the_client() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        saved_session(&store, "a", 12 * MIN);
        saved_session(&store, "b", 50 * MIN);
        let sessions = store.list().unwrap();
        let games = list_games(&store, &sessions, &[], None);
        let ids: Vec<_> = games.iter().map(|g| g.session_id.as_deref()).collect();
        assert_eq!(ids, [Some("b"), Some("a")]);
        // nothing is known about the outcome or the queue: only the recording speaks
        assert_eq!(games[0].win, None);
        assert_eq!((games[0].queue_id, games[0].champion_id), (0, 0));
        assert_eq!(games[0].duration_secs, 1800);
        assert_eq!(games[0].game_id, 50 * MIN);
        assert_eq!(games[0].account.as_deref(), Some("Me#EUW"));
        assert!(store.load_game_summary("b").unwrap().is_none());
    }

    #[test]
    fn a_recorded_game_out_of_the_history_keeps_what_the_client_said() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        saved_session(&store, "a", 12 * MIN);
        let sessions = store.list().unwrap();
        list_games(&store, &sessions, &[game(1, 10 * MIN)], Some("Me#EUW"));
        // the game has left the history of the client
        let games = list_games(&store, &sessions, &[game(2, 900 * MIN)], Some("Me#EUW"));
        let kept = games.iter().find(|g| g.session_id.is_some()).unwrap();
        assert_eq!(
            (kept.game_id, kept.queue_id, kept.win),
            (1, 420, Some(true))
        );
        assert_eq!((kept.kills, kept.deaths, kept.assists), (1, 2, 3));
        assert_eq!(
            games.len(),
            2,
            "the unrecorded game of the history is listed too"
        );
        assert_eq!(games[0].game_id, 2, "most recent first");
    }

    #[test]
    fn a_session_is_used_once_and_closest_wins() {
        let matches = [game(1, 100 * MIN), game(2, 105 * MIN)];
        let sessions = [session("a", 106 * MIN, true, false)];
        let g = merged(&matches, &sessions);
        assert!(g[0].session_id.is_none());
        assert_eq!(g[1].session_id.as_deref(), Some("a"));
    }
}
