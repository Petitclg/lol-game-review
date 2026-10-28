//! Builds a [`Session`] incrementally from successive Live Client API samples.

use crate::models::{AllGameData, SpellName};
use lgr_core::session::{anchor_from_sample, ItemInfo, PlayerSnapshot, Session, Snapshot};

/// Minimum game-time gap between two stored snapshots.
const SNAPSHOT_INTERVAL_SECS: f64 = 3.0;

pub struct SessionTracker {
    session: Session,
    record_start_ms: u64,
    best_anchor: Option<f64>,
}

impl SessionTracker {
    #[must_use]
    pub fn new(id: String, record_start_ms: u64, recording_file: Option<String>) -> Self {
        Self {
            session: Session {
                id,
                started_at_ms: record_start_ms,
                player_aliases: Vec::new(),
                video_anchor_secs: 0.0,
                recording_file,
                last_game_time_secs: 0.0,
                events: Vec::new(),
                snapshots: Vec::new(),
            },
            record_start_ms,
            best_anchor: None,
        }
    }

    pub fn observe(&mut self, data: &AllGameData, now_ms: u64) {
        let game_time = data.game_data.game_time;
        let s = &mut self.session;

        if s.player_aliases.is_empty() {
            s.player_aliases = data.aliases();
        }

        let anchor = anchor_from_sample(game_time, now_ms, self.record_start_ms);
        let best = self.best_anchor.map_or(anchor, |b| b.max(anchor));
        self.best_anchor = Some(best);
        s.video_anchor_secs = best;

        s.last_game_time_secs = s.last_game_time_secs.max(game_time);
        if data.events.events.len() >= s.events.len() {
            s.events.clone_from(&data.events.events);
        }

        let due = s
            .snapshots
            .last()
            .is_none_or(|last| game_time - last.game_time_secs >= SNAPSHOT_INTERVAL_SECS);
        if due {
            s.snapshots.push(snapshot(data));
        }
    }

    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    #[must_use]
    pub fn finish(self) -> Session {
        self.session
    }
}

fn snapshot(data: &AllGameData) -> Snapshot {
    let me = &data.active_player;
    let players = data
        .all_players
        .iter()
        .map(|p| PlayerSnapshot {
            name: p.display_name().to_owned(),
            champion: p.champion_key(),
            team: p.team.clone(),
            is_me: !me.riot_id.is_empty() && p.riot_id == me.riot_id
                || (me.riot_id.is_empty() && p.summoner_name == me.summoner_name),
            level: p.level,
            kills: p.scores.kills,
            deaths: p.scores.deaths,
            assists: p.scores.assists,
            items: p
                .items
                .iter()
                .map(|i| ItemInfo {
                    id: i.item_id,
                    name: i.display_name.clone(),
                })
                .collect(),
            spells: [
                &p.summoner_spells.summoner_spell_one.display_name,
                &p.summoner_spells.summoner_spell_two.display_name,
            ]
            .into_iter()
            .filter(|n| !n.is_empty())
            .cloned()
            .collect(),
            spell_keys: [
                &p.summoner_spells.summoner_spell_one,
                &p.summoner_spells.summoner_spell_two,
            ]
            .into_iter()
            .filter_map(SpellName::key)
            .collect(),
        })
        .collect();
    Snapshot {
        game_time_secs: data.game_data.game_time,
        players,
        my_gold: me.current_gold,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(game_time: f64, events: usize) -> AllGameData {
        let evs: Vec<String> = (0..events)
            .map(|i| {
                format!(r#"{{"EventID":{i},"EventName":"ChampionKill","EventTime":{game_time},"KillerName":"Me","VictimName":"X"}}"#)
            })
            .collect();
        let json = format!(
            r#"{{"activePlayer":{{"riotId":"Me#EUW","riotIdGameName":"Me","summonerName":"Me#EUW","currentGold":742.5}},
            "allPlayers":[{{"riotId":"Me#EUW","summonerName":"Me#EUW","championName":"Ahri","team":"ORDER","level":3,
              "scores":{{"kills":1,"deaths":0,"assists":0}},"items":[{{"itemID":1055,"displayName":"Doran's Blade"}}],
              "summonerSpells":{{"summonerSpellOne":{{"displayName":"Saut éclair","rawDisplayName":"GeneratedTip_SummonerSpell_SummonerFlash_DisplayName"}},"summonerSpellTwo":{{"displayName":"Ignite"}}}}}},
             {{"riotId":"Foe#EUW","summonerName":"Foe#EUW","championName":"Zed","team":"CHAOS","level":3}}],
            "events":{{"Events":[{}]}},"gameData":{{"gameTime":{game_time}}}}}"#,
            evs.join(",")
        );
        AllGameData::parse(&json).unwrap()
    }

    #[test]
    fn captures_identity_and_snapshot_details() {
        let mut t = SessionTracker::new("s".into(), 1_000, None);
        t.observe(&data(10.0, 0), 11_000);
        let s = t.session();
        assert_eq!(s.player_aliases, ["Me#EUW", "Me"]);
        let snap = &s.snapshots[0];
        assert!(snap.players[0].is_me && !snap.players[1].is_me);
        assert_eq!(snap.players[0].items[0].id, 1055);
        assert_eq!(snap.players[0].spells, ["Saut éclair", "Ignite"]);
        assert_eq!(snap.players[0].spell_keys, ["SummonerFlash"]);
        assert_eq!(snap.my_gold, Some(742.5));
    }

    #[test]
    fn anchor_keeps_best_estimate() {
        let mut t = SessionTracker::new("s".into(), 0, None);
        t.observe(&data(10.0, 0), 10_500); // anchor -0.5
        t.observe(&data(20.0, 0), 20_100); // anchor -0.1 (lower latency)
        t.observe(&data(30.0, 0), 31_000); // anchor -1.0
        assert!((t.session().video_anchor_secs - -0.1).abs() < 1e-9);
    }

    #[test]
    fn snapshots_are_throttled() {
        let mut t = SessionTracker::new("s".into(), 0, None);
        for (i, gt) in [10.0, 11.0, 12.5, 13.0, 14.0].iter().enumerate() {
            t.observe(&data(*gt, 0), 10_000 + i as u64 * 1000);
        }
        let times: Vec<_> = t
            .session()
            .snapshots
            .iter()
            .map(|s| s.game_time_secs)
            .collect();
        assert_eq!(times, [10.0, 13.0]);
    }

    #[test]
    fn events_are_replaced_by_the_latest_full_list() {
        let mut t = SessionTracker::new("s".into(), 0, None);
        t.observe(&data(10.0, 1), 10_000);
        t.observe(&data(20.0, 3), 20_000);
        assert_eq!(t.session().events.len(), 3);
        // a shorter list (transient glitch) must not drop recorded events
        t.observe(&data(30.0, 0), 30_000);
        assert_eq!(t.session().events.len(), 3);
    }

    #[test]
    fn tracks_last_game_time_monotonically() {
        let mut t = SessionTracker::new("s".into(), 0, Some("rec.mkv".into()));
        t.observe(&data(50.0, 0), 50_000);
        t.observe(&data(40.0, 0), 51_000);
        assert!((t.finish().last_game_time_secs - 50.0).abs() < 1e-9);
    }
}
