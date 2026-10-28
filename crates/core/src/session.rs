//! Recorded game session: timeline, per-player snapshots and clip generation.

use crate::clips::{build_windows, ClipConfig};
use crate::events::{classify, Highlight, RawLiveEvent, Theme};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemInfo {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerSnapshot {
    pub name: String,
    /// Data Dragon champion id (e.g. `Ahri`, `MonkeyKing`).
    pub champion: String,
    pub team: String,
    pub is_me: bool,
    pub level: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub items: Vec<ItemInfo>,
    pub spells: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct AbilityRanks {
    pub q: u32,
    pub w: u32,
    pub e: u32,
    pub r: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub game_time_secs: f64,
    pub players: Vec<PlayerSnapshot>,
    pub ability_ranks: Option<AbilityRanks>,
    pub ability_haste: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// Wall-clock time (epoch ms) at which the recording started.
    pub started_at_ms: u64,
    /// Names under which the Live Client API refers to the tracked player.
    pub player_aliases: Vec<String>,
    /// Game time (secs) corresponding to video time 0.
    pub video_anchor_secs: f64,
    pub recording_file: Option<String>,
    pub last_game_time_secs: f64,
    pub events: Vec<RawLiveEvent>,
    pub snapshots: Vec<Snapshot>,
}

/// A highlight clip, as exposed to the UI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub theme: Theme,
    pub label: String,
    pub game_time_secs: f64,
    /// Position in the source video.
    pub start_secs: f64,
    pub end_secs: f64,
    pub file: Option<String>,
    pub players: Vec<PlayerSnapshot>,
    pub ability_ranks: Option<AbilityRanks>,
    pub ability_haste: Option<f64>,
}

/// Converts a game time to a position in the recorded video (never negative).
#[must_use]
pub fn to_video_time(game_time_secs: f64, anchor_secs: f64) -> f64 {
    (game_time_secs - anchor_secs).max(0.0)
}

/// Estimates the game time at video time 0 from one sample.
///
/// A sample is `game_time` observed at wall-clock `sample_ms`; the recording started
/// at `record_start_ms`. Because the HTTP round trip makes `game_time` slightly stale,
/// the best estimate over several samples is the maximum.
#[must_use]
pub fn anchor_from_sample(game_time_secs: f64, sample_ms: u64, record_start_ms: u64) -> f64 {
    let elapsed = sample_ms.saturating_sub(record_start_ms) as f64 / 1000.0;
    game_time_secs - elapsed
}

/// A clip must show at least this much video to be worth keeping.
const MIN_VISIBLE_SECS: f64 = 1.0;

fn snapshot_at(snapshots: &[Snapshot], game_time_secs: f64) -> Option<&Snapshot> {
    snapshots
        .iter()
        .rev()
        .find(|s| s.game_time_secs <= game_time_secs)
        .or_else(|| snapshots.first())
}

fn theme_key(theme: Theme) -> &'static str {
    match theme {
        Theme::Kill => "kill",
        Theme::Death => "death",
        Theme::Assist => "assist",
        Theme::Objective => "objective",
        Theme::Turret => "turret",
    }
}

fn group_label(highlights: &[Highlight]) -> String {
    match highlights {
        [only] => only.label.clone(),
        [first, rest @ ..] => format!("{} ×{}", first.label, rest.len() + 1),
        [] => String::new(),
    }
}

/// Builds the clips of a session, grouped by theme; within a theme, highlights whose
/// windows overlap (e.g. a multikill) share a single clip. File paths are left empty.
#[must_use]
pub fn build_clips(session: &Session, cfg: ClipConfig) -> Vec<Clip> {
    let highlights = classify(&session.events, &session.player_aliases);
    let themes = [
        Theme::Kill,
        Theme::Death,
        Theme::Assist,
        Theme::Objective,
        Theme::Turret,
    ];
    let mut clips = Vec::new();
    for theme in themes {
        let of_theme: Vec<Highlight> = highlights
            .iter()
            .filter(|h| h.theme == theme)
            .cloned()
            .collect();
        for (i, w) in build_windows(&of_theme, cfg).into_iter().enumerate() {
            // The recording may have started mid-game: nothing to show before it.
            if w.end_secs - session.video_anchor_secs <= MIN_VISIBLE_SECS {
                continue;
            }
            let first = &w.highlights[0];
            let snap = snapshot_at(&session.snapshots, first.time_secs);
            clips.push(Clip {
                id: format!("{}-{i}", theme_key(theme)),
                theme,
                label: group_label(&w.highlights),
                game_time_secs: first.time_secs,
                start_secs: to_video_time(w.start_secs, session.video_anchor_secs),
                end_secs: to_video_time(w.end_secs, session.video_anchor_secs),
                file: None,
                players: snap.map(|s| s.players.clone()).unwrap_or_default(),
                ability_ranks: snap.and_then(|s| s.ability_ranks),
                ability_haste: snap.and_then(|s| s.ability_haste),
            });
        }
    }
    clips.sort_by(|a, b| a.game_time_secs.total_cmp(&b.game_time_secs));
    clips
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(id: u32, name: &str, t: f64, killer: &str, victim: &str) -> RawLiveEvent {
        RawLiveEvent {
            event_id: id,
            event_name: name.into(),
            event_time: t,
            killer_name: Some(killer.into()),
            victim_name: Some(victim.into()),
            assisters: vec![],
            dragon_type: None,
        }
    }

    fn snap(t: f64, level: u32) -> Snapshot {
        Snapshot {
            game_time_secs: t,
            players: vec![PlayerSnapshot {
                name: "Me#EUW".into(),
                champion: "Ahri".into(),
                team: "ORDER".into(),
                is_me: true,
                level,
                kills: 0,
                deaths: 0,
                assists: 0,
                items: vec![],
                spells: vec![],
            }],
            ability_ranks: Some(AbilityRanks {
                q: 1,
                w: 1,
                e: 1,
                r: 0,
            }),
            ability_haste: Some(10.0),
        }
    }

    fn session(events: Vec<RawLiveEvent>, anchor: f64) -> Session {
        Session {
            id: "s".into(),
            started_at_ms: 0,
            player_aliases: vec!["Me".into()],
            video_anchor_secs: anchor,
            recording_file: None,
            last_game_time_secs: 1000.0,
            events,
            snapshots: vec![snap(0.0, 1), snap(100.0, 6), snap(200.0, 9)],
        }
    }

    #[test]
    fn video_time_subtracts_anchor_and_clamps() {
        assert!((to_video_time(130.0, 30.0) - 100.0).abs() < 1e-9);
        assert!(to_video_time(10.0, 30.0).abs() < 1e-9);
    }

    #[test]
    fn anchor_accounts_for_elapsed_recording_time() {
        let a = anchor_from_sample(70.0, 1_000_000 + 10_000, 1_000_000);
        assert!((a - 60.0).abs() < 1e-9);
    }

    #[test]
    fn clips_use_latest_snapshot_before_event() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe")], 0.0);
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].players[0].level, 6);
        assert_eq!(clips[0].ability_haste, Some(10.0));
    }

    #[test]
    fn clip_positions_are_relative_to_video() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe")], 50.0);
        let c = &build_clips(&s, ClipConfig::default())[0];
        assert!((c.start_secs - 92.0).abs() < 1e-9);
        assert!((c.end_secs - 104.0).abs() < 1e-9);
        assert!((c.game_time_secs - 150.0).abs() < 1e-9);
    }

    #[test]
    fn same_theme_overlaps_merge_with_count_label() {
        let s = session(
            vec![
                ev(1, "ChampionKill", 150.0, "Me", "A"),
                ev(2, "ChampionKill", 155.0, "Me", "B"),
            ],
            0.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].label, "Kill ×2");
    }

    #[test]
    fn different_themes_stay_separate_and_sorted() {
        let s = session(
            vec![
                ev(2, "ChampionKill", 160.0, "Foe", "Me"),
                ev(1, "ChampionKill", 150.0, "Me", "Foe"),
            ],
            0.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        let themes: Vec<_> = clips.iter().map(|c| c.theme).collect();
        assert_eq!(themes, [Theme::Kill, Theme::Death]);
        assert_eq!(clips[0].id, "kill-0");
    }

    #[test]
    fn highlights_before_recording_started_are_dropped() {
        let s = session(
            vec![
                ev(1, "ChampionKill", 40.0, "Me", "Foe"),
                ev(2, "ChampionKill", 200.0, "Me", "Foe"),
            ],
            100.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 1);
        assert!((clips[0].game_time_secs - 200.0).abs() < 1e-9);
    }

    #[test]
    fn no_highlight_gives_no_clip() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "A", "B")], 0.0);
        assert!(build_clips(&s, ClipConfig::default()).is_empty());
    }

    #[test]
    fn session_roundtrips_through_json() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe")], 5.0);
        let back: Session = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }
}
