//! Subset of the `GET /liveclientdata/allgamedata` payload.

use lgr_core::events::RawLiveEvent;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllGameData {
    pub active_player: ActivePlayer,
    pub all_players: Vec<Player>,
    pub events: EventList,
    pub game_data: GameData,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EventList {
    #[serde(rename = "Events")]
    pub events: Vec<RawLiveEvent>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    pub game_time: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivePlayer {
    #[serde(default)]
    pub riot_id: String,
    #[serde(default)]
    pub riot_id_game_name: String,
    #[serde(default)]
    pub summoner_name: String,
    /// Unspent gold; only exposed for the local player.
    #[serde(default)]
    pub current_gold: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    #[serde(default)]
    pub riot_id: String,
    #[serde(default)]
    pub summoner_name: String,
    #[serde(default)]
    pub champion_name: String,
    #[serde(default)]
    pub raw_champion_name: String,
    #[serde(default)]
    pub team: String,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub scores: Scores,
    #[serde(default)]
    pub items: Vec<RawItem>,
    #[serde(default)]
    pub summoner_spells: SummonerSpells,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
pub struct Scores {
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RawItem {
    #[serde(rename = "itemID")]
    pub item_id: u32,
    #[serde(default, rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummonerSpells {
    #[serde(default)]
    pub summoner_spell_one: SpellName,
    #[serde(default)]
    pub summoner_spell_two: SpellName,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpellName {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub raw_display_name: String,
}

const RAW_SPELL_PREFIX: &str = "GeneratedTip_SummonerSpell_";
const RAW_SPELL_SUFFIX: &str = "_DisplayName";

impl SpellName {
    /// Stable id such as `SummonerFlash`, independent of the game language.
    pub fn key(&self) -> Option<String> {
        self.raw_display_name
            .strip_prefix(RAW_SPELL_PREFIX)
            .and_then(|s| s.strip_suffix(RAW_SPELL_SUFFIX))
            .filter(|k| !k.is_empty())
            .map(str::to_owned)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("invalid live data: {0}")]
    Json(#[from] serde_json::Error),
}

impl AllGameData {
    pub fn parse(json: &str) -> Result<Self, ParseError> {
        Ok(serde_json::from_str(json)?)
    }

    /// Names under which events may refer to the active player.
    pub fn aliases(&self) -> Vec<String> {
        let a = &self.active_player;
        let mut names: Vec<String> = Vec::new();
        for n in [&a.riot_id, &a.riot_id_game_name, &a.summoner_name] {
            if !n.is_empty() && !names.contains(n) {
                names.push(n.clone());
            }
        }
        names
    }

    pub fn has_game_ended(&self) -> bool {
        self.events.events.iter().any(|e| e.event_name == "GameEnd")
    }
}

const RAW_PREFIX: &str = "game_character_displayname_";

impl Player {
    /// Data Dragon champion id, e.g. `MonkeyKing`; falls back to the display name.
    pub fn champion_key(&self) -> String {
        self.raw_champion_name
            .strip_prefix(RAW_PREFIX)
            .filter(|k| !k.is_empty())
            .unwrap_or(&self.champion_name)
            .to_owned()
    }

    pub fn display_name(&self) -> &str {
        if self.riot_id.is_empty() {
            &self.summoner_name
        } else {
            &self.riot_id
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub const SAMPLE: &str = r#"{
      "activePlayer":{"riotId":"Me#EUW","riotIdGameName":"Me","summonerName":"Me#EUW"},
      "allPlayers":[
        {"riotId":"Me#EUW","summonerName":"Me#EUW","championName":"Wukong","rawChampionName":"game_character_displayname_MonkeyKing",
         "team":"ORDER","level":8,"scores":{"kills":2,"deaths":1,"assists":3},
         "items":[{"itemID":1001,"displayName":"Boots","count":1}],
         "summonerSpells":{"summonerSpellOne":{"displayName":"Flash"},"summonerSpellTwo":{"displayName":"Ignite"}}},
        {"riotId":"Foe#EUW","championName":"Ahri","team":"CHAOS"}
      ],
      "events":{"Events":[{"EventID":0,"EventName":"GameStart","EventTime":0.0}]},
      "gameData":{"gameMode":"CLASSIC","gameTime":123.4}
    }"#;

    #[test]
    fn parses_sample() {
        let d = AllGameData::parse(SAMPLE).unwrap();
        assert!((d.game_data.game_time - 123.4).abs() < 1e-9);
        assert_eq!(d.all_players.len(), 2);
        assert_eq!(d.all_players[0].items[0].item_id, 1001);
    }

    #[test]
    fn champion_key_prefers_raw_name() {
        let d = AllGameData::parse(SAMPLE).unwrap();
        assert_eq!(d.all_players[0].champion_key(), "MonkeyKing");
        assert_eq!(d.all_players[1].champion_key(), "Ahri");
    }

    #[test]
    fn aliases_are_deduplicated_and_non_empty() {
        let d = AllGameData::parse(SAMPLE).unwrap();
        assert_eq!(d.aliases(), ["Me#EUW", "Me"]);
    }

    #[test]
    fn detects_game_end() {
        let mut d = AllGameData::parse(SAMPLE).unwrap();
        assert!(!d.has_game_ended());
        d.events.events.push(RawLiveEvent {
            event_id: 9,
            event_name: "GameEnd".into(),
            event_time: 1800.0,
            killer_name: None,
            victim_name: None,
            assisters: vec![],
            dragon_type: None,
        });
        assert!(d.has_game_ended());
    }

    #[test]
    fn spell_key_is_language_independent() {
        let s = SpellName {
            display_name: "Saut éclair".into(),
            raw_display_name: "GeneratedTip_SummonerSpell_SummonerFlash_DisplayName".into(),
        };
        assert_eq!(s.key().as_deref(), Some("SummonerFlash"));
        assert_eq!(SpellName::default().key(), None);
    }

    #[test]
    fn rejects_garbage() {
        assert!(AllGameData::parse("{}").is_err());
    }
}
