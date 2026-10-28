//! Merges the LCU match history with locally recorded sessions.

use lgr_core::store::SessionMeta;
use lgr_lcu::MatchSummary;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameEntry {
    pub game_id: u64,
    pub created_ms: u64,
    pub duration_secs: u32,
    pub queue_id: u32,
    pub champion_id: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub win: bool,
    pub session_id: Option<String>,
    pub clips_ready: bool,
}

/// A recording starts when the live API becomes available, i.e. after loading,
/// so it begins shortly after the game was created.
const MAX_LEAD_MS: i64 = 15 * 60 * 1000;
const MAX_LAG_MS: i64 = 60 * 1000;

/// Pairs games with recorded sessions started within the expected window after the
/// game's creation. Closest pairs are assigned first and a session is used once.
#[must_use]
pub fn merge_games(matches: &[MatchSummary], sessions: &[SessionMeta]) -> Vec<GameEntry> {
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
                kills: m.kills,
                deaths: m.deaths,
                assists: m.assists,
                win: m.win,
                session_id: session.map(|s| s.id.clone()),
                clips_ready: session.is_some_and(|s| s.clips_ready),
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
        }
    }

    const MIN: u64 = 60_000;

    #[test]
    fn pairs_session_started_shortly_after_creation() {
        let g = merge_games(&[game(1, 10 * MIN)], &[session("a", 12 * MIN, true, false)]);
        assert_eq!(g[0].session_id.as_deref(), Some("a"));
        assert!(!g[0].clips_ready);
    }

    #[test]
    fn ignores_sessions_outside_the_window() {
        let far = merge_games(&[game(1, 10 * MIN)], &[session("a", 40 * MIN, true, false)]);
        let before = merge_games(&[game(1, 10 * MIN)], &[session("a", 5 * MIN, true, false)]);
        assert!(far[0].session_id.is_none() && before[0].session_id.is_none());
    }

    #[test]
    fn sessions_without_video_or_clips_are_not_offered() {
        let g = merge_games(
            &[game(1, 10 * MIN)],
            &[session("a", 11 * MIN, false, false)],
        );
        assert!(g[0].session_id.is_none());
    }

    #[test]
    fn clips_only_session_stays_available() {
        let g = merge_games(&[game(1, 10 * MIN)], &[session("a", 11 * MIN, false, true)]);
        assert!(g[0].clips_ready);
    }

    #[test]
    fn a_session_is_used_once_and_closest_wins() {
        let matches = [game(1, 100 * MIN), game(2, 105 * MIN)];
        let sessions = [session("a", 106 * MIN, true, false)];
        let g = merge_games(&matches, &sessions);
        assert!(g[0].session_id.is_none());
        assert_eq!(g[1].session_id.as_deref(), Some("a"));
    }
}
