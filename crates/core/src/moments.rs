//! Decisive moments of a game, whoever takes part in them: aces, and large swings of
//! estimated team gold.
//!
//! Riot exposes no gold for other players, so a swing is **estimated** from the events
//! that award a well-known amount of gold to a team (champion kills, turrets, inhibitors,
//! Baron). The amounts are orders of magnitude, not exact figures; what matters is that a
//! burst of them in favour of one team marks a decisive moment.

use crate::events::{Highlight, RawLiveEvent, Theme};
use crate::session::{resolve, PlayerSnapshot};

const ORDER: &str = "ORDER";
const CHAOS: &str = "CHAOS";

/// Estimated gold gained by a team for one champion kill.
const KILL_GOLD: f64 = 300.0;
/// Estimated gold gained by the team that destroys a turret.
const TURRET_GOLD: f64 = 500.0;
/// Estimated gold gained by the team that destroys an inhibitor.
const INHIBITOR_GOLD: f64 = 250.0;
/// Gold gained by the team that kills Baron (300 for each of its five players).
const BARON_GOLD: f64 = 1500.0;

/// Gold events are added up over this many seconds.
pub const SWING_WINDOW_SECS: f64 = 30.0;
/// Net gold, in favour of one team, from which a window is a decisive moment.
pub const SWING_THRESHOLD_GOLD: f64 = 1500.0;
/// Minimum time between two swing moments, so one fight is reported once.
const SWING_COOLDOWN_SECS: f64 = 45.0;
/// A swing this close to an ace is the same fight and is left to the ace.
const ACE_MERGE_SECS: f64 = 30.0;

fn other_team(team: &str) -> &'static str {
    if team == ORDER {
        CHAOS
    } else {
        ORDER
    }
}

/// Team of a Live Client internal structure name such as `Turret_T1_L_03_A` or
/// `Barracks_T2_C1`: `T1` is the blue side (`ORDER`), `T2` the red side (`CHAOS`).
fn structure_team(name: &str) -> Option<&'static str> {
    match name.split('_').nth(1)? {
        "T1" => Some(ORDER),
        "T2" => Some(CHAOS),
        _ => None,
    }
}

fn team_of<'a>(players: &'a [PlayerSnapshot], name: Option<&str>) -> Option<&'a str> {
    resolve(players, name?).map(|p| p.team.as_str())
}

/// Team that benefits from an event and the estimated gold it gains, if the event
/// awards any.
fn gold_gain(e: &RawLiveEvent, players: &[PlayerSnapshot]) -> Option<(&'static str, f64)> {
    let static_team = |t: &str| if t == ORDER { ORDER } else { CHAOS };
    match e.event_name.as_str() {
        "ChampionKill" => {
            let winner = team_of(players, e.victim_name.as_deref())
                .map(other_team)
                .or_else(|| team_of(players, e.killer_name.as_deref()).map(static_team))?;
            Some((winner, KILL_GOLD))
        }
        "TurretKilled" => Some((
            other_team(structure_team(e.turret_killed.as_deref()?)?),
            TURRET_GOLD,
        )),
        "InhibKilled" => Some((
            other_team(structure_team(e.inhib_killed.as_deref()?)?),
            INHIBITOR_GOLD,
        )),
        "BaronKill" => Some((
            static_team(team_of(players, e.killer_name.as_deref())?),
            BARON_GOLD,
        )),
        _ => None,
    }
}

/// `1800` -> `1 800`.
fn format_gold(gold: i64) -> String {
    let digits = gold.abs().to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(' ');
        }
        out.push(c);
    }
    out
}

fn ace_highlight(e: &RawLiveEvent, my_team: Option<&str>) -> Option<Highlight> {
    if e.event_name != "Ace" {
        return None;
    }
    let label = match (e.acing_team.as_deref(), my_team) {
        (Some(acing), Some(mine)) if acing == mine => "Ace de votre équipe",
        (Some(_), Some(_)) => "Ace adverse",
        _ => "Ace",
    };
    Some(Highlight {
        event_id: e.event_id,
        time_secs: e.event_time,
        theme: Theme::KeyMoment,
        label: label.to_owned(),
        killer: e.acer.clone(),
        victim: None,
        assisters: Vec::new(),
        caster: None,
    })
}

/// Decisive moments of a game, sorted by time: aces, and bursts of estimated gold that
/// favour one team. `players` is the roster (names and teams); the local player's team
/// only decides the wording of the labels.
#[must_use]
pub fn key_moments(events: &[RawLiveEvent], players: &[PlayerSnapshot]) -> Vec<Highlight> {
    let my_team = players.iter().find(|p| p.is_me).map(|p| p.team.as_str());
    let aces: Vec<Highlight> = events
        .iter()
        .filter_map(|e| ace_highlight(e, my_team))
        .collect();

    let mut gains: Vec<(&RawLiveEvent, &'static str, f64)> = events
        .iter()
        .filter_map(|e| gold_gain(e, players).map(|(team, gold)| (e, team, gold)))
        .collect();
    gains.sort_by(|a, b| a.0.event_time.total_cmp(&b.0.event_time));

    let mut swings: Vec<Highlight> = Vec::new();
    let mut last_swing = f64::NEG_INFINITY;
    for (i, (event, _, _)) in gains.iter().enumerate() {
        let now = event.event_time;
        if now < last_swing + SWING_COOLDOWN_SECS {
            continue;
        }
        let net: f64 = gains[..=i]
            .iter()
            .filter(|(e, _, _)| e.event_time > now - SWING_WINDOW_SECS)
            .map(|(_, team, gold)| if *team == ORDER { *gold } else { -*gold })
            .sum();
        if net.abs() < SWING_THRESHOLD_GOLD {
            continue;
        }
        let winner = if net > 0.0 { ORDER } else { CHAOS };
        if aces
            .iter()
            .any(|a| (a.time_secs - now).abs() <= ACE_MERGE_SECS)
        {
            last_swing = now;
            continue;
        }
        last_swing = now;
        swings.push(swing_highlight(event, winner, net.abs(), my_team));
    }

    let mut out = aces;
    out.extend(swings);
    out.sort_by(|a, b| {
        a.time_secs
            .total_cmp(&b.time_secs)
            .then(a.event_id.cmp(&b.event_id))
    });
    out
}

fn swing_highlight(
    event: &RawLiveEvent,
    winner: &str,
    gold: f64,
    my_team: Option<&str>,
) -> Highlight {
    let amount = format_gold(gold.round() as i64);
    let label = match my_team {
        Some(mine) if mine == winner => format!("Écart d'or +{amount}"),
        Some(_) => format!("Écart d'or −{amount}"),
        None => format!("Écart d'or {amount}"),
    };
    Highlight {
        event_id: event.event_id,
        time_secs: event.event_time,
        theme: Theme::KeyMoment,
        label,
        killer: None,
        victim: None,
        assisters: Vec::new(),
        caster: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(name: &str, team: &str, is_me: bool) -> PlayerSnapshot {
        PlayerSnapshot {
            name: name.into(),
            champion: "Ahri".into(),
            team: team.into(),
            is_me,
            level: 10,
            kills: 0,
            deaths: 0,
            assists: 0,
            items: vec![],
            spells: vec![],
            spell_keys: vec![],
        }
    }

    fn roster() -> Vec<PlayerSnapshot> {
        vec![
            player("Me#EUW", "ORDER", true),
            player("Ally", "ORDER", false),
            player("Foe", "CHAOS", false),
            player("Foe2", "CHAOS", false),
        ]
    }

    fn kill(id: u32, t: f64, killer: &str, victim: &str) -> RawLiveEvent {
        RawLiveEvent {
            event_id: id,
            event_name: "ChampionKill".into(),
            event_time: t,
            killer_name: Some(killer.into()),
            victim_name: Some(victim.into()),
            ..RawLiveEvent::default()
        }
    }

    fn named(id: u32, t: f64, name: &str, field: Option<&str>) -> RawLiveEvent {
        let mut e = RawLiveEvent {
            event_id: id,
            event_name: name.into(),
            event_time: t,
            ..RawLiveEvent::default()
        };
        match name {
            "TurretKilled" => e.turret_killed = field.map(Into::into),
            "InhibKilled" => e.inhib_killed = field.map(Into::into),
            _ => {}
        }
        e
    }

    fn ace(id: u32, t: f64, team: &str) -> RawLiveEvent {
        RawLiveEvent {
            event_id: id,
            event_name: "Ace".into(),
            event_time: t,
            acer: Some("Ally".into()),
            acing_team: Some(team.into()),
            ..RawLiveEvent::default()
        }
    }

    /// Five kills by `Foe` in 20 s: +1500 for CHAOS.
    fn chaos_fight(start: f64) -> Vec<RawLiveEvent> {
        (0..5)
            .map(|i| kill(10 + i, start + f64::from(i) * 5.0, "Foe", "Ally"))
            .collect()
    }

    #[test]
    fn an_ace_is_a_key_moment_worded_from_the_local_players_side() {
        let events = [ace(1, 500.0, "ORDER"), ace(2, 900.0, "CHAOS")];
        let m = key_moments(&events, &roster());
        let labels: Vec<_> = m.iter().map(|h| h.label.as_str()).collect();
        assert_eq!(labels, ["Ace de votre équipe", "Ace adverse"]);
        assert!(m.iter().all(|h| h.theme == Theme::KeyMoment));
        assert_eq!(m[0].killer.as_deref(), Some("Ally"));
    }

    #[test]
    fn an_ace_without_a_known_local_team_is_just_an_ace() {
        let m = key_moments(&[ace(1, 500.0, "ORDER")], &[]);
        assert_eq!(m[0].label, "Ace");
    }

    #[test]
    fn a_burst_of_kills_for_one_team_is_a_gold_swing() {
        let m = key_moments(&chaos_fight(600.0), &roster());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].label, "Écart d'or −1 500");
        assert!(
            (m[0].time_secs - 620.0).abs() < 1e-9,
            "at the event that crosses it"
        );
    }

    #[test]
    fn a_swing_in_favour_of_the_local_team_is_positive() {
        let events: Vec<_> = (0..5)
            .map(|i| kill(10 + i, 300.0 + f64::from(i) * 4.0, "Ally", "Foe"))
            .collect();
        let m = key_moments(&events, &roster());
        assert_eq!(m[0].label, "Écart d'or +1 500");
    }

    #[test]
    fn exchanged_kills_cancel_out() {
        let mut events = Vec::new();
        for i in 0..6 {
            let (k, v) = if i % 2 == 0 {
                ("Foe", "Ally")
            } else {
                ("Ally", "Foe")
            };
            events.push(kill(i, 100.0 + f64::from(i) * 4.0, k, v));
        }
        assert!(key_moments(&events, &roster()).is_empty());
    }

    #[test]
    fn scattered_kills_are_not_a_swing() {
        let events: Vec<_> = (0..8)
            .map(|i| kill(i, 100.0 + f64::from(i) * 60.0, "Foe", "Ally"))
            .collect();
        assert!(key_moments(&events, &roster()).is_empty());
    }

    #[test]
    fn a_turret_counts_for_the_team_that_destroys_it() {
        // T2 is the red side's: the blue side destroyed it
        let mut events = vec![named(1, 700.0, "TurretKilled", Some("Turret_T2_L_03_A"))];
        events.extend((0..4).map(|i| kill(10 + i, 705.0 + f64::from(i) * 3.0, "Ally", "Foe")));
        let m = key_moments(&events, &roster());
        assert_eq!(m[0].label, "Écart d'or +1 700");
    }

    #[test]
    fn inhibitors_and_baron_are_valued() {
        let events = [
            named(1, 1000.0, "InhibKilled", Some("Barracks_T1_C1")),
            RawLiveEvent {
                event_id: 2,
                event_name: "BaronKill".into(),
                event_time: 1010.0,
                killer_name: Some("Foe".into()),
                ..RawLiveEvent::default()
            },
        ];
        // inhibitor of the blue side (+250 CHAOS) then Baron by CHAOS (+1500)
        let m = key_moments(&events, &roster());
        assert_eq!(m[0].label, "Écart d'or −1 750");
    }

    #[test]
    fn structures_and_events_with_unknown_names_are_ignored() {
        let events = [
            named(1, 700.0, "TurretKilled", Some("Weird")),
            named(2, 710.0, "TurretKilled", None),
            named(3, 720.0, "DragonKill", None),
            kill(4, 730.0, "Minion_T100L0S1", "Nobody"),
        ];
        assert!(key_moments(&events, &roster()).is_empty());
    }

    #[test]
    fn a_kill_by_a_minion_counts_against_the_victims_team() {
        let events: Vec<_> = (0..5)
            .map(|i| kill(i, 100.0 + f64::from(i) * 5.0, "SRU_Baron", "Ally"))
            .collect();
        let m = key_moments(&events, &roster());
        assert_eq!(m[0].label, "Écart d'or −1 500");
    }

    #[test]
    fn one_fight_is_reported_once() {
        let mut events = chaos_fight(600.0);
        events.push(kill(99, 625.0, "Foe2", "Ally"));
        events.push(kill(100, 630.0, "Foe2", "Me"));
        assert_eq!(key_moments(&events, &roster()).len(), 1);
    }

    #[test]
    fn two_distant_fights_are_two_moments() {
        let mut events = chaos_fight(600.0);
        events.extend(chaos_fight(1200.0));
        let m = key_moments(&events, &roster());
        assert_eq!(m.len(), 2);
        assert!(m[0].time_secs < m[1].time_secs);
    }

    #[test]
    fn an_ace_replaces_the_swing_of_the_same_fight() {
        let mut events = chaos_fight(600.0);
        events.push(ace(50, 622.0, "CHAOS"));
        let m = key_moments(&events, &roster());
        assert_eq!(m.len(), 1);
        assert_eq!(m[0].label, "Ace adverse");
    }

    #[test]
    fn a_swing_far_from_an_ace_is_kept() {
        let mut events = chaos_fight(600.0);
        events.push(ace(50, 900.0, "ORDER"));
        assert_eq!(key_moments(&events, &roster()).len(), 2);
    }

    #[test]
    fn results_are_sorted_by_time() {
        let mut events = vec![ace(50, 300.0, "ORDER")];
        events.extend(chaos_fight(600.0));
        events.reverse();
        let times: Vec<_> = key_moments(&events, &roster())
            .iter()
            .map(|h| h.time_secs)
            .collect();
        assert!(times.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(times.len(), 2);
    }

    #[test]
    fn gold_is_written_with_thousand_separators() {
        assert_eq!(format_gold(0), "0");
        assert_eq!(format_gold(950), "950");
        assert_eq!(format_gold(1500), "1 500");
        assert_eq!(format_gold(12_345), "12 345");
        assert_eq!(format_gold(1_234_567), "1 234 567");
    }
}
