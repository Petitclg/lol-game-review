//! Recorded game session: timeline, per-player snapshots and clip generation.

use crate::clips::{build_windows, ClipConfig};
use crate::events::{classify, Highlight, RawLiveEvent, Theme};
use crate::moments::key_moments;
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

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
    /// `true` once the anchor comes from a running game clock or was checked against
    /// the recording. Sessions saved by older versions may have been anchored on the
    /// frozen clock of the loading screen.
    #[serde(default)]
    pub anchor_verified: bool,
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
    /// Every theme of the clip, `theme` first: several once clips of different themes
    /// have been merged. Empty for clips stored by older versions.
    #[serde(default)]
    pub themes: Vec<Theme>,
    /// The highlights shown by the clip, by time. Annotations are attached to them.
    #[serde(default)]
    pub parts: Vec<ClipPart>,
}

/// A highlight shown by a clip. Its id depends on the theme and the game time only, so it
/// survives a regeneration and a change of the clip windows or of the merges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipPart {
    pub id: String,
    pub theme: Theme,
    pub label: String,
    pub game_time_secs: f64,
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
pub(crate) fn resolve<'a>(
    players: &'a [PlayerSnapshot],
    event_name: &str,
) -> Option<&'a PlayerSnapshot> {
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
    theme.key()
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
    post_roll_secs: 11.0,
};

/// A Flash is detected about a second after it is cast; what matters is the fight it
/// serves, which follows it.
const FLASH_CONFIG: ClipConfig = ClipConfig {
    pre_roll_secs: 6.0,
    post_roll_secs: 9.0,
};

/// A decisive moment is the end of a fight: show how it built up.
const KEY_MOMENT_CONFIG: ClipConfig = ClipConfig {
    pre_roll_secs: 20.0,
    post_roll_secs: 8.0,
};

/// Casts of the local player read from the recorded HUD, in game time (seconds).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Casts {
    pub ultimate: Vec<f64>,
    /// Casts of summoner spell one (key D) and two (key F).
    pub summoner: [Vec<f64>; 2],
}

impl Casts {
    /// Only ultimates (for sessions analysed before summoner spells were read).
    #[must_use]
    pub fn ultimates(times: &[f64]) -> Self {
        Self {
            ultimate: times.to_vec(),
            summoner: [Vec::new(), Vec::new()],
        }
    }
}

const FLASH_KEY: &str = "SummonerFlash";

/// Which summoner spell slot (0 = first, 1 = second) holds the local player's Flash, or
/// `None` when the player has none or the slots cannot be told apart.
#[must_use]
pub fn flash_slot(session: &Session) -> Option<usize> {
    let me = session
        .snapshots
        .iter()
        .rev()
        .find_map(|s| s.players.iter().find(|p| p.is_me))?;
    // Both keys must be known, or their position would not tell the slot.
    if me.spell_keys.len() != 2 {
        return None;
    }
    me.spell_keys.iter().position(|k| k == FLASH_KEY)
}

fn cast_highlights(session: &Session, times: &[f64], theme: Theme, label: &str) -> Vec<Highlight> {
    times
        .iter()
        .enumerate()
        .map(|(i, &t)| Highlight {
            event_id: u32::try_from(i).unwrap_or(u32::MAX),
            time_secs: t,
            theme,
            label: label.to_owned(),
            killer: None,
            victim: None,
            assisters: Vec::new(),
            caster: session.player_aliases.first().cloned(),
        })
        .collect()
}

/// Highlights that do not involve the local player: aces and gold swings.
fn key_moment_highlights(session: &Session) -> Vec<Highlight> {
    let roster = session
        .snapshots
        .last()
        .map(|s| s.players.as_slice())
        .unwrap_or_default();
    key_moments(&session.events, roster)
}

/// Builds the clips of a session, grouped by theme; within a theme, highlights whose
/// windows overlap (e.g. a multikill) share a single clip; clips of any theme that
/// overlap are then merged. File paths are left empty.
#[must_use]
pub fn build_clips(session: &Session, cfg: ClipConfig) -> Vec<Clip> {
    build_clips_with(session, cfg, &Casts::default())
}

/// Like [`build_clips`], also producing ultimate and Flash clips for the casts read
/// from the recorded HUD.
#[must_use]
pub fn build_clips_with(session: &Session, cfg: ClipConfig, casts: &Casts) -> Vec<Clip> {
    sorted(merge_overlapping(build_clips_unmerged(session, cfg, casts)))
}

fn sorted(mut clips: Vec<Clip>) -> Vec<Clip> {
    clips.sort_by(|a, b| a.game_time_secs.total_cmp(&b.game_time_secs));
    clips
}

/// Like [`build_clips_with`] but without merging clips of different themes: the clips
/// as versions up to 0.1.9 cut them. Clips stored by those versions are matched against
/// these.
#[must_use]
pub fn build_clips_unmerged(session: &Session, cfg: ClipConfig, casts: &Casts) -> Vec<Clip> {
    let mut highlights = classify(&session.events, &session.player_aliases);
    highlights.extend(cast_highlights(
        session,
        &casts.ultimate,
        Theme::Ultimate,
        "Ultimate",
    ));
    if let Some(slot) = flash_slot(session) {
        highlights.extend(cast_highlights(
            session,
            &casts.summoner[slot],
            Theme::Flash,
            "Flash",
        ));
    }
    highlights.extend(key_moment_highlights(session));
    let themes = [
        Theme::Kill,
        Theme::Death,
        Theme::Assist,
        Theme::Objective,
        Theme::Turret,
        Theme::Ultimate,
        Theme::Flash,
        Theme::KeyMoment,
    ];
    let mut clips = Vec::new();
    for theme in themes {
        let mut of_theme: Vec<Highlight> = highlights
            .iter()
            .filter(|h| h.theme == theme)
            .cloned()
            .collect();
        of_theme.sort_by(|a, b| a.time_secs.total_cmp(&b.time_secs));
        let theme_cfg = match theme {
            Theme::Ultimate => ULTIMATE_CONFIG,
            Theme::Flash => FLASH_CONFIG,
            Theme::KeyMoment => KEY_MOMENT_CONFIG,
            _ => cfg,
        };
        for (i, w) in build_windows(&of_theme, theme_cfg).into_iter().enumerate() {
            // The recording may have started mid-game: nothing to show before it.
            if w.end_secs - session.video_anchor_secs <= MIN_VISIBLE_SECS {
                continue;
            }
            let first = &w.highlights[0];
            let snap = snapshot_at(&session.snapshots, first.time_secs);
            let players = snap.map(|s| s.players.clone()).unwrap_or_default();
            let label = group_label(&w.highlights);
            clips.push(Clip {
                id: format!("{}-{i}", theme_key(theme)),
                theme,
                label: label.clone(),
                game_time_secs: first.time_secs,
                start_secs: to_video_time(w.start_secs, session.video_anchor_secs),
                end_secs: to_video_time(w.end_secs, session.video_anchor_secs),
                file: None,
                involved: involved_players(&w.highlights, &players),
                my_gold: snap.and_then(|s| s.my_gold),
                players,
                themes: vec![theme],
                parts: vec![ClipPart {
                    id: part_id(theme, first.time_secs),
                    theme,
                    label,
                    game_time_secs: first.time_secs,
                }],
            });
        }
    }
    sorted(clips)
}

/// Stable identifier of the highlight of `theme` at `game_time_secs`.
fn part_id(theme: Theme, game_time_secs: f64) -> String {
    format!("{}-{game_time_secs:.1}", theme_key(theme))
}

/// A merged clip never grows beyond this duration: chaining every fight of a long
/// skirmish would give a clip nobody wants to watch.
const MAX_MERGED_SECS: f64 = 60.0;

/// The theme a merged clip is shown under first (colour, title): what hurts or counts
/// most comes first.
const THEME_PRIORITY: [Theme; 8] = [
    Theme::Death,
    Theme::Kill,
    Theme::Assist,
    Theme::Ultimate,
    Theme::Flash,
    Theme::Objective,
    Theme::Turret,
    Theme::KeyMoment,
];

/// At most this many part labels are spelled out in the label of a merged clip.
const MERGED_LABELS: usize = 3;

fn merge_involved(into: &mut Vec<Involved>, other: &[Involved]) {
    for o in other {
        match into.iter_mut().find(|i| i.name == o.name) {
            Some(i) => {
                for role in &o.roles {
                    if !i.roles.contains(role) {
                        i.roles.push(*role);
                    }
                }
            }
            None => into.push(o.clone()),
        }
    }
}

/// One clip made of `group`, which is sorted by start and overlaps from one clip to the
/// next. The snapshot (players, items, gold) is the one of the earliest highlight.
fn merge_group(group: Vec<Clip>, number: usize) -> Clip {
    let mut parts: Vec<ClipPart> = group.iter().flat_map(|c| c.parts.clone()).collect();
    parts.sort_by(|a, b| a.game_time_secs.total_cmp(&b.game_time_secs));
    let themes: Vec<Theme> = THEME_PRIORITY
        .into_iter()
        .filter(|t| parts.iter().any(|p| p.theme == *t))
        .collect();
    let labels: Vec<&str> = parts
        .iter()
        .take(MERGED_LABELS)
        .map(|p| p.label.as_str())
        .collect();
    let hidden = parts.len().saturating_sub(MERGED_LABELS);
    let mut label = labels.join(" + ");
    if hidden > 0 {
        let _ = write!(label, " +{hidden}");
    }

    let start_secs = group.iter().map(|c| c.start_secs).fold(f64::MAX, f64::min);
    let end_secs = group.iter().map(|c| c.end_secs).fold(f64::MIN, f64::max);
    let mut by_time = group;
    by_time.sort_by(|a, b| a.game_time_secs.total_cmp(&b.game_time_secs));
    let mut involved = Vec::new();
    for c in &by_time {
        merge_involved(&mut involved, &c.involved);
    }
    let earliest = by_time.swap_remove(0);
    Clip {
        id: format!("seq-{number}"),
        theme: themes.first().copied().unwrap_or(earliest.theme),
        themes,
        label,
        game_time_secs: earliest.game_time_secs,
        start_secs,
        end_secs,
        file: None,
        players: earliest.players,
        involved,
        my_gold: earliest.my_gold,
        parts,
    }
}

/// Merges the clips, whatever their themes, whose windows touch or overlap, so that a
/// kill followed by a death, with an ultimate in between, is a single clip. A merged
/// clip lasts at most [`MAX_MERGED_SECS`]; clips that do not overlap are left as they are.
fn merge_overlapping(mut clips: Vec<Clip>) -> Vec<Clip> {
    clips.sort_by(|a, b| a.start_secs.total_cmp(&b.start_secs));
    let mut out = Vec::new();
    let mut group: Vec<Clip> = Vec::new();
    let mut group_end = f64::MIN;
    let mut merged = 0;
    let mut flush = |group: &mut Vec<Clip>, out: &mut Vec<Clip>| match group.len() {
        0 => {}
        1 => out.append(group),
        _ => {
            out.push(merge_group(std::mem::take(group), merged));
            merged += 1;
        }
    };
    for clip in clips {
        let start = group.first().map_or(clip.start_secs, |c| c.start_secs);
        let fits =
            clip.start_secs <= group_end && clip.end_secs.max(group_end) - start <= MAX_MERGED_SECS;
        if !fits {
            flush(&mut group, &mut out);
            group_end = f64::MIN;
        }
        group_end = group_end.max(clip.end_secs);
        group.push(clip);
    }
    flush(&mut group, &mut out);
    out
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
            ..RawLiveEvent::default()
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
            anchor_verified: true,
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
        let clips = build_clips_with(
            &s,
            ClipConfig::default(),
            &Casts::ultimates(&[200.0, 205.0, 400.0]),
        );
        assert_eq!(clips.len(), 2);
        assert!(clips.iter().all(|c| c.theme == Theme::Ultimate));
        assert_eq!(clips[0].label, "Ultimate ×2");
        assert!((clips[0].start_secs - 195.0).abs() < 1e-9);
        assert!((clips[0].end_secs - 216.0).abs() < 1e-9);
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
        let clips = build_clips_with(&s, ClipConfig::default(), &Casts::ultimates(&[50.0, 300.0]));
        assert_eq!(clips.len(), 1);
        assert!((clips[0].game_time_secs - 300.0).abs() < 1e-9);
    }

    #[test]
    fn ultimate_clips_interleave_with_event_clips_by_time() {
        let s = session(vec![ev(1, "ChampionKill", 150.0, "Me", "Foe")], 0.0);
        let clips = build_clips_with(
            &s,
            ClipConfig::default(),
            &Casts::ultimates(&[100.0, 300.0]),
        );
        let themes: Vec<_> = clips.iter().map(|c| c.theme).collect();
        assert_eq!(themes, [Theme::Ultimate, Theme::Kill, Theme::Ultimate]);
    }

    /// A session whose local player has the given summoner spell keys.
    fn session_with_spells(keys: &[&str], events: Vec<RawLiveEvent>) -> Session {
        let mut s = session(events, 0.0);
        for snap in &mut s.snapshots {
            snap.players[0].spell_keys = keys.iter().map(|k| (*k).to_owned()).collect();
        }
        s
    }

    #[test]
    fn the_flash_slot_is_the_position_of_flash_among_the_two_spells() {
        assert_eq!(
            flash_slot(&session_with_spells(
                &["SummonerFlash", "SummonerDot"],
                vec![]
            )),
            Some(0)
        );
        assert_eq!(
            flash_slot(&session_with_spells(
                &["SummonerDot", "SummonerFlash"],
                vec![]
            )),
            Some(1)
        );
        assert_eq!(
            flash_slot(&session_with_spells(
                &["SummonerHeal", "SummonerDot"],
                vec![]
            )),
            None
        );
    }

    #[test]
    fn the_flash_slot_is_unknown_when_a_spell_key_is_missing() {
        assert_eq!(
            flash_slot(&session_with_spells(&["SummonerFlash"], vec![])),
            None
        );
        assert_eq!(flash_slot(&session_with_spells(&[], vec![])), None);
        assert_eq!(flash_slot(&session(vec![], 0.0)), None);
    }

    #[test]
    fn flash_clips_use_the_casts_of_the_slot_holding_flash() {
        let s = session_with_spells(&["SummonerDot", "SummonerFlash"], vec![]);
        let casts = Casts {
            ultimate: vec![],
            summoner: [vec![111.0], vec![222.0, 500.0]],
        };
        let clips = build_clips_with(&s, ClipConfig::default(), &casts);
        assert_eq!(clips.len(), 2);
        assert!(clips.iter().all(|c| c.theme == Theme::Flash));
        assert_eq!(clips[0].id, "flash-0");
        assert_eq!(clips[0].label, "Flash");
        assert!((clips[0].start_secs - 216.0).abs() < 1e-9);
        assert!((clips[0].end_secs - 231.0).abs() < 1e-9);
        assert_eq!(
            clips[0].involved,
            [Involved {
                name: "Me#EUW".into(),
                roles: vec![Role::Caster]
            }]
        );
    }

    #[test]
    fn no_flash_clip_without_flash_or_without_a_known_slot() {
        let casts = Casts {
            ultimate: vec![],
            summoner: [vec![111.0], vec![222.0]],
        };
        for keys in [
            &["SummonerHeal", "SummonerDot"][..],
            &["SummonerFlash"][..],
            &[][..],
        ] {
            let s = session_with_spells(keys, vec![]);
            assert!(build_clips_with(&s, ClipConfig::default(), &casts).is_empty());
        }
    }

    #[test]
    fn key_moments_are_clips_even_when_the_local_player_is_not_involved() {
        let events: Vec<_> = (0..5)
            .map(|i| ev(i, "ChampionKill", 100.0 + f64::from(i) * 5.0, "Foe", "Ally"))
            .collect();
        let s = session(events, 0.0);
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].theme, Theme::KeyMoment);
        assert_eq!(clips[0].id, "key_moment-0");
        assert_eq!(clips[0].label, "Écart d'or −1 500");
        assert!(
            (clips[0].start_secs - 100.0).abs() < 1e-9,
            "20 s before the 5th kill"
        );
        assert!((clips[0].end_secs - 128.0).abs() < 1e-9);
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
        assert!((c.end_secs - 108.0).abs() < 1e-9);
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
    fn different_themes_far_apart_stay_separate_and_sorted() {
        let s = session(
            vec![
                ev(2, "ChampionKill", 300.0, "Foe", "Me"),
                ev(1, "ChampionKill", 150.0, "Me", "Foe"),
            ],
            0.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        let themes: Vec<_> = clips.iter().map(|c| c.theme).collect();
        assert_eq!(themes, [Theme::Kill, Theme::Death]);
        assert_eq!(clips[0].id, "kill-0");
        assert_eq!(clips[0].themes, [Theme::Kill]);
        assert_eq!(clips[0].parts.len(), 1);
        assert_eq!(clips[0].parts[0].id, "kill-150.0");
    }

    #[test]
    fn a_kill_and_a_death_that_overlap_make_one_clip() {
        let s = session(
            vec![
                ev(2, "ChampionKill", 160.0, "Foe", "Me"),
                ev(1, "ChampionKill", 150.0, "Me", "Foe"),
            ],
            0.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 1);
        let c = &clips[0];
        assert_eq!(c.id, "seq-0");
        assert_eq!(c.theme, Theme::Death, "the death is what the clip is about");
        assert_eq!(c.themes, [Theme::Death, Theme::Kill]);
        assert_eq!(c.game_time_secs, 150.0);
        assert_eq!((c.start_secs, c.end_secs), (142.0, 168.0));
        let ids: Vec<_> = c.parts.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["kill-150.0", "death-160.0"]);
        assert_eq!(
            c.label,
            format!("{} + {}", c.parts[0].label, c.parts[1].label)
        );
    }

    #[test]
    fn a_merged_clip_gathers_the_roles_of_both_highlights() {
        let s = session(
            vec![
                ev(1, "ChampionKill", 150.0, "Me", "Foe"),
                ev(2, "ChampionKill", 160.0, "Foe", "Me"),
            ],
            0.0,
        );
        let c = &build_clips(&s, ClipConfig::default())[0];
        let roles = |name: &str| {
            c.involved
                .iter()
                .find(|i| i.name.starts_with(name))
                .map(|i| i.roles.clone())
        };
        assert_eq!(roles("Me"), Some(vec![Role::Killer, Role::Victim]));
        assert_eq!(roles("Foe"), Some(vec![Role::Victim, Role::Killer]));
    }

    #[test]
    fn clips_that_only_touch_are_merged_and_clips_with_a_gap_are_not() {
        // windows are 16 s long: 150 -> [142, 158]; 166 -> [158, 174] touches; 190 does not
        let s = session(
            vec![
                ev(1, "ChampionKill", 150.0, "Me", "A"),
                ev(2, "ChampionKill", 166.0, "B", "Me"),
                ev(3, "ChampionKill", 200.0, "Me", "C"),
            ],
            0.0,
        );
        let clips = build_clips(&s, ClipConfig::default());
        assert_eq!(clips.len(), 2);
        assert_eq!(clips[0].parts.len(), 2);
        assert_eq!(clips[1].id, "kill-1");
    }

    #[test]
    fn a_chain_of_fights_is_cut_so_that_no_merged_clip_exceeds_the_limit() {
        let events = (0..8u32)
            .map(|i| {
                let t = 100.0 + 12.0 * f64::from(i);
                if i % 2 == 0 {
                    ev(i + 1, "ChampionKill", t, "Me", "Foe")
                } else {
                    ev(i + 1, "ChampionKill", t, "Foe", "Me")
                }
            })
            .collect();
        let clips = build_clips(&session(events, 0.0), ClipConfig::default());
        assert!(clips.len() >= 2, "{} clip(s)", clips.len());
        assert!(clips
            .iter()
            .all(|c| c.end_secs - c.start_secs <= MAX_MERGED_SECS));
        let parts: usize = clips.iter().map(|c| c.parts.len()).sum();
        assert_eq!(parts, 8, "no highlight is lost");
    }

    #[test]
    fn a_long_merged_clip_names_only_its_first_highlights() {
        let events = (0..4u32)
            .map(|i| {
                let t = 100.0 + 12.0 * f64::from(i);
                ev(
                    i + 1,
                    "ChampionKill",
                    t,
                    if i % 2 == 0 { "Me" } else { "Foe" },
                    if i % 2 == 0 { "Foe" } else { "Me" },
                )
            })
            .collect();
        let clips = build_clips(&session(events, 0.0), ClipConfig::default());
        assert_eq!(clips.len(), 1);
        assert!(clips[0].label.ends_with(" +1"), "{}", clips[0].label);
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
