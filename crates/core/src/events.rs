//! Classification of Live Client Data API events into highlight themes.

use serde::{Deserialize, Serialize};

/// Raw event as returned by `GET /liveclientdata/eventdata` (subset of fields).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RawLiveEvent {
    #[serde(rename = "EventID")]
    pub event_id: u32,
    pub event_name: String,
    pub event_time: f64,
    #[serde(default)]
    pub killer_name: Option<String>,
    #[serde(default)]
    pub victim_name: Option<String>,
    #[serde(default)]
    pub assisters: Vec<String>,
    #[serde(default)]
    pub dragon_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct EventEnvelope {
    #[serde(rename = "Events")]
    events: Vec<RawLiveEvent>,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid event payload: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parses the JSON body of the event endpoint.
pub fn parse_events(json: &str) -> Result<Vec<RawLiveEvent>, ParseError> {
    Ok(serde_json::from_str::<EventEnvelope>(json)?.events)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    Kill,
    Death,
    Assist,
    Objective,
    Turret,
    /// The local player's own ultimates, detected from the recorded HUD.
    Ultimate,
}

/// A highlight in which the tracked player is involved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Highlight {
    pub event_id: u32,
    pub time_secs: f64,
    pub theme: Theme,
    pub label: String,
    /// Names as given by the Live Client API (game name, without tag).
    #[serde(default)]
    pub killer: Option<String>,
    #[serde(default)]
    pub victim: Option<String>,
    #[serde(default)]
    pub assisters: Vec<String>,
    /// Player casting the ability (ultimate highlights only).
    #[serde(default)]
    pub caster: Option<String>,
}

/// Returns the highlights of the player among `events`, sorted by time.
///
/// `aliases` are all the names under which the Live Client API may refer to the
/// player (Riot ID, game name, summoner name).
/// Objectives and turrets count only when the player got the last hit or an assist.
/// Ultimates are not exposed by the Live Client API and are out of scope here.
#[must_use]
pub fn classify<S: AsRef<str>>(events: &[RawLiveEvent], aliases: &[S]) -> Vec<Highlight> {
    let mut out: Vec<Highlight> = events
        .iter()
        .filter_map(|e| classify_one(e, aliases))
        .collect();
    out.sort_by(|a, b| {
        a.time_secs
            .total_cmp(&b.time_secs)
            .then(a.event_id.cmp(&b.event_id))
    });
    out
}

fn is<S: AsRef<str>>(name: Option<&str>, aliases: &[S]) -> bool {
    name.is_some_and(|n| aliases.iter().any(|a| a.as_ref() == n))
}

fn classify_one<S: AsRef<str>>(e: &RawLiveEvent, aliases: &[S]) -> Option<Highlight> {
    let killer = is(e.killer_name.as_deref(), aliases);
    let assisted = e.assisters.iter().any(|a| is(Some(a), aliases));
    let (theme, label) = match e.event_name.as_str() {
        "ChampionKill" if killer => (Theme::Kill, "Kill"),
        "ChampionKill" if is(e.victim_name.as_deref(), aliases) => (Theme::Death, "Death"),
        "ChampionKill" if assisted => (Theme::Assist, "Assist"),
        "DragonKill" if killer || assisted => (Theme::Objective, "Dragon"),
        "BaronKill" if killer || assisted => (Theme::Objective, "Baron"),
        "HeraldKill" if killer || assisted => (Theme::Objective, "Herald"),
        "HordeKill" if killer || assisted => (Theme::Objective, "Void Grub"),
        "AtakhanKill" if killer || assisted => (Theme::Objective, "Atakhan"),
        "TurretKilled" if killer || assisted => (Theme::Turret, "Turret"),
        "InhibKilled" if killer || assisted => (Theme::Turret, "Inhibitor"),
        _ => return None,
    };
    Some(Highlight {
        event_id: e.event_id,
        time_secs: e.event_time,
        theme,
        label: label.to_owned(),
        killer: e.killer_name.clone(),
        victim: e.victim_name.clone(),
        assisters: e.assisters.clone(),
        caster: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAYLOAD: &str = r#"{"Events":[
      {"EventID":0,"EventName":"GameStart","EventTime":0.0},
      {"EventID":1,"EventName":"ChampionKill","EventTime":120.5,"KillerName":"Me","VictimName":"Foe","Assisters":["Ally"]},
      {"EventID":2,"EventName":"ChampionKill","EventTime":200.0,"KillerName":"Foe","VictimName":"Me","Assisters":[]},
      {"EventID":3,"EventName":"ChampionKill","EventTime":150.0,"KillerName":"Ally","VictimName":"Foe2","Assisters":["Me"]},
      {"EventID":4,"EventName":"ChampionKill","EventTime":160.0,"KillerName":"Ally","VictimName":"Foe3","Assisters":[]},
      {"EventID":5,"EventName":"DragonKill","EventTime":600.0,"KillerName":"Me","Assisters":[],"DragonType":"Fire"},
      {"EventID":6,"EventName":"TurretKilled","EventTime":700.0,"KillerName":"Ally","Assisters":["Me"]},
      {"EventID":7,"EventName":"BaronKill","EventTime":1500.0,"KillerName":"Foe","Assisters":[]}
    ]}"#;

    fn highlights() -> Vec<Highlight> {
        classify(&parse_events(PAYLOAD).unwrap(), &["Me"])
    }

    #[test]
    fn matches_any_alias() {
        let events = parse_events(PAYLOAD).unwrap();
        let h = classify(&events, &["Me#EUW", "Me"]);
        assert_eq!(h.len(), 5);
        assert!(classify(&events, &["Nobody"]).is_empty());
    }

    #[test]
    fn parses_all_events() {
        assert_eq!(parse_events(PAYLOAD).unwrap().len(), 8);
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(parse_events("not json").is_err());
    }

    #[test]
    fn keeps_only_player_events_sorted_by_time() {
        let themes: Vec<_> = highlights().iter().map(|h| h.theme).collect();
        assert_eq!(
            themes,
            [
                Theme::Kill,
                Theme::Assist,
                Theme::Death,
                Theme::Objective,
                Theme::Turret
            ]
        );
    }

    #[test]
    fn ignores_events_without_player() {
        assert!(highlights()
            .iter()
            .all(|h| h.event_id != 4 && h.event_id != 7));
    }
}
