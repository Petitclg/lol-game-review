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
    /// Stable summoner spell ids (e.g. `SummonerFlash`); empty for older recordings.
    #[serde(default)]
    pub spell_keys: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub game_time_secs: f64,
    pub players: Vec<PlayerSnapshot>,
    /// Unspent gold of the local player (the API exposes no other player's gold).
    #[serde(default)]
    pub my_gold: Option<f64>,
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
    /// Players taking part in the highlight; empty when unknown (older clips).
    #[serde(default)]
    pub involved: Vec<Involved>,
    #[serde(default)]
    pub my_gold: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Killer,
    Victim,
    Assist,
    /// Caster of an ultimate.
    Caster,
}

/// A player involved in a highlight; `name` equals `PlayerSnapshot::name`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Involved {
    pub name: String,
    pub roles: Vec<Role>,
}

/// Maps a name from a Live Client event (game name, no tag) to a player of the snapshot.
fn resolve<'a>(players: &'a [PlayerSnapshot], event_name: &str) -> Option<&'a PlayerSnapshot> {
    players.iter().find(|p| p.name == event_name).or_else(|| {
        players
            .iter()
            .find(|p| p.name.split('#').next() == Some(event_name))
    })
}

/// Players taking part in the given highlights. Names that are not players
/// (minions, turrets) are ignored; a player's roles are merged across highlights.
#[must_use]
pub fn involved_players(highlights: &[Highlight], players: &[PlayerSnapshot]) -> Vec<Involved> {
    let mut out: Vec<Involved> = Vec::new();
    let mut add = |event_name: &str, role: Role| {
        let Some(p) = resolve(players, event_name) else {
            return;
        };
        match out.iter_mut().find(|i| i.name == p.name) {
            Some(i) if !i.roles.contains(&role) => i.roles.push(role),
            Some(_) => {}
            None => out.push(Involved {
                name: p.name.clone(),
                roles: vec![role],
            }),
        }
    };
    for h in highlights {
        if let Some(c) = &h.caster {
            add(c, Role::Caster);
        }
        if let Some(k) = &h.killer {
            add(k, Role::Killer);
        }
        if let Some(v) = &h.victim {
            add(v, Role::Victim);
        }
        for a in &h.assisters {
            add(a, Role::Assist);
        }
    }
    out
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
        Theme::Ultimate => "ultimate",
    }
}

fn group_label(highlights: &[Highlight]) -> String {
    match highlights {
        [only] => only.label.clone(),
        [first, rest @ ..] => format!("{} ×{}", first.label, rest.len() + 1),
        [] => String::new(),
    }
}

/// An ultimate is detected about a second after it is cast and its effect lasts
/// longer than a kill, so the window starts later and ends later.
const ULTIMATE_CONFIG: ClipConfig = ClipConfig {
    pre_roll_secs: 5.0,
    post_roll_secs: 8.0,
};

/// Highlights for the local player's ultimates cast at the given game times.
fn ultimate_highlights(session: &Session, cast_game_secs: &[f64]) -> Vec<Highlight> {
    cast_game_secs
        .iter()
        .enumerate()
        .map(|(i, &t)| Highlight {
            event_id: u32::try_from(i).unwrap_or(u32::MAX),
            time_secs: t,
            theme: Theme::Ultimate,
            label: "Ultimate".to_owned(),
            killer: None,
            victim: None,
            assisters: Vec::new(),
            caster: session.player_aliases.first().cloned(),
        })
        .collect()
}

/// Builds the clips of a session, grouped by theme; within a theme, highlights whose
/// windows overlap (e.g. a multikill) share a single clip. File paths are left empty.
#[must_use]
pub fn build_clips(session: &Session, cfg: ClipConfig) -> Vec<Clip> {
    build_clips_with(session, cfg, &[])
}

/// Like [`build_clips`], also producing ultimate clips for casts detected at
/// `ultimate_game_secs` (game time, seconds).
#[must_use]
pub fn build_clips_with(
    session: &Session,
    cfg: ClipConfig,
    ultimate_game_secs: &[f64],
) -> Vec<Clip> {
    let mut highlights = classify(&session.events, &session.player_aliases);
    highlights.extend(ultimate_highlights(session, ultimate_game_secs));
    let themes = [
        Theme::Kill,
        Theme::Death,
        Theme::Assist,
        Theme::Objective,
        Theme::Turret,
        Theme::Ultimate,
    ];
    let mut clips = Vec::new();
    for theme in themes {
        let mut of_theme: Vec<Highlight> = highlights
            .iter()
            .filter(|h| h.theme == theme)
            .cloned()
            .collect();
        of_theme.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
        let theme_cfg = if theme == Theme::Ultimate {
            ULTIMATE_CONFIG
        } else {
            cfg
        };
        for (i, w) in build_windows(&of_theme, theme_cfg).into_iter().enumerate() {
            // The recording may have started mid-game: nothing to show before it.
            if w.end_secs - session.video_anchor_secs <= MIN_VISIBLE_SECS {
                continue;
            }
            let first = &w.highlights[0];
            let snap = snapshot_at(&session.snapshots, first.time_secs);
            let players = snap.map(|s| s.players.clone()).unwrap_or_default();
            clips.push(Clip {
                id: format!("{}-{i}", theme_key(theme)),
                theme,
                label: group_label(&w.highlights),
                game_time_secs: first.time_secs,
                start_secs: to_video_time(w.start_secs, session.video_anchor_secs),
                end_secs: to_video_time(w.end_secs, session.video_anchor_secs),
                file: None,
                involved: involved_players(&w.highlights, &players),
                my_gold: snap.and_then(|s| s.my_gold),
                players,
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

    fn player(name: &str, team: &str, is_me: bool, level: u32) -> PlayerSnapshot {
        PlayerSnapshot {
            name: name.into(),
            champion: "Ahri".into(),
            team: team.into(),
            is_me,
            level,
            kills: 0,
            deaths: 0,
            assists: 0,
            items: vec![],
            spells: vec![],
            spell_keys: vec![],
        }
    }

    fn snap(t: f64, level: u32) -> Snapshot {
        Snapshot {
            game_time_secs: t,
            players: vec![
                player("Me#EUW", "ORDER", true, level),
                player("Ally#EUW", "ORDER", false, level),
                player("Foe#EUW", "CHAOS", false, level),
            ],
            my_gold: Some(500.0),
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
    }

    #[test]
    fn sessions_saved_with_legacy_ability_fields_still_load() {
        let json = r#"{"id":"s","startedAtMs":1,"playerAliases":[],"videoAnchorSecs":0.0,
            "recordingFile":null,"lastGameTimeSecs":1.0,"events":[],
            "snapshots":[{"gameTimeSecs":1.0,"players":[],"abilityRanks":{"q":1,"w":1,"e":1,"r":0},"abilityHaste":10.0}]}"#;
        let s: Session = serde_json::from_str(json).unwrap();
        assert_eq!(s.snapshots.len(), 1);
    }

    #[test]
    fn involved_players_resolve_tagless_event_names() {
        let mut e = ev(1, "ChampionKill", 150.0, "Me", "Foe");
        e.assisters = vec!["Ally".into()];
        let s = session(vec![e], 0.0);
        let c = &build_clips(&s, ClipConfig::default())[0];
        assert_eq!(
            c.involved,
            [
                Involved {
                    name: "Me#EUW".into(),
                    roles: vec![Role::Killer]
                },
                Involved {
                    name: "Foe#EUW".into(),
                    roles: vec![Role::Victim]
                },
                Involved {
                    name: "Ally#EUW".into(),
                    roles: vec![Role::Assist]
                },
            ]
        );
        assert_eq!(c.my_gold, Some(500.0));
    }

    #[test]
    fn ultimate_clips_are_built_from_cast_times_with_their_own_window() {
        let s = session(vec![], 0.0);
        let clips = build_clips_with(&s, ClipConfig::default(), &[200.0, 205.0, 400.0]);
        assert_eq!(clips.len(), 2);
        assert!(clips.iter().all(|c| c.theme == Theme::Ultimate));
        assert_eq!(clips[0].label, "Ultimate ×2");
        assert!((clips[0].start_secs - 195.0).abs() < 1e-9);
        assert!((clips[0].end_secs - 213.0).abs() < 1e-9);
        assert_eq!(clips[1].id, "ultimate-1");
        assert_eq!(
            clips[0].involved,
            [Involved {
                name: "Me#EUW".into(),
                roles: vec![Role::Caster]
            }]
        );
    }

    #[test]
    fn ultimates_cast_before_recording_started_are_dropped() {
        let s = session(vec![], 100.0);
        let clips = build_clips_with(&s, ClipConfig::default(), &[50.0, 300.0]);
        assert_eq!(clips.len(), 1);
        assert!((clips[0].game_time_secs - 300.0).abs() < 1e-9);
    }

    #[test]
    fn ultimate_clips_interleave_with_event_clips_by_time() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe")], 0.0);
        let clips = build_clips_with(&s, ClipConfig::default(), &[100.0, 300.0]);
        let themes: Vec<_> = clips.iter().map(|c| c.theme).collect();
        assert_eq!(themes, [Theme::Ultimate, Theme::Kill, Theme::Ultimate]);
    }

    #[test]
    fn non_player_killers_are_ignored() {
        let s = session(
            vec![ev(1, "ChampionKill", 150.0, "Minion_T200L0S1", "Me")],
            0.0,
        );
        let c = &build_clips(&s, ClipConfig::default())[0];
        let names: Vec<_> = c.involved.iter().map(|i| i.name.as_str()).collect();
        assert_eq!(names, ["Me#EUW"]);
    }

    #[test]
    fn roles_are_merged_across_a_multikill() {
        let mut second = ev(2, "ChampionKill", 155.0, "Me", "Foe");
        second.assisters = vec!["Ally".into()];
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe"), second], 0.0);
        let c = &build_clips(&s, ClipConfig::default())[0];
        assert_eq!(c.involved.len(), 3);
        let me = c.involved.iter().find(|i| i.name == "Me#EUW").unwrap();
        assert_eq!(me.roles, [Role::Killer]);
    }

    #[test]
    fn clips_saved_before_involvement_existed_still_load() {
        let json = r#"{"id":"kill-0","theme":"kill","label":"Kill","gameTimeSecs":1.0,
            "startSecs":0.0,"endSecs":1.0,"file":null,"players":[]}"#;
        let c: Clip = serde_json::from_str(json).unwrap();
        assert!(c.involved.is_empty() && c.my_gold.is_none());
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
