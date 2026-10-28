//! Data models returned by the LCU API (only the fields we need).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Summoner {
    pub puuid: String,
    #[serde(default)]
    pub game_name: String,
    #[serde(default)]
    pub tag_line: String,
    pub summoner_level: u32,
}

impl Summoner {
    /// `GameName#TAG`, the Riot ID shown in game.
    #[must_use]
    pub fn riot_id(&self) -> String {
        format!("{}#{}", self.game_name, self.tag_line)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchSummary {
    pub game_id: u64,
    pub created_ms: u64,
    pub duration_secs: u32,
    pub queue_id: u32,
    pub champion_id: u32,
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub win: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct MatchHistory {
    pub games: GameList,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GameList {
    pub games: Vec<RawGame>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawGame {
    pub game_id: u64,
    pub game_creation: u64,
    pub game_duration: u32,
    pub queue_id: u32,
    pub participants: Vec<RawParticipant>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RawParticipant {
    pub champion_id: u32,
    pub stats: RawStats,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RawStats {
    pub kills: u32,
    pub deaths: u32,
    pub assists: u32,
    pub win: bool,
}

impl RawGame {
    /// The LCU history lists the current summoner as the only participant;
    /// games without one are skipped.
    pub(crate) fn into_summary(self) -> Option<MatchSummary> {
        let p = self.participants.into_iter().next()?;
        Some(MatchSummary {
            game_id: self.game_id,
            created_ms: self.game_creation,
            duration_secs: self.game_duration,
            queue_id: self.queue_id,
            champion_id: p.champion_id,
            kills: p.stats.kills,
            deaths: p.stats.deaths,
            assists: p.stats.assists,
            win: p.stats.win,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summoner_riot_id() {
        let s: Summoner = serde_json::from_str(
            r#"{"puuid":"p","gameName":"Me","tagLine":"EUW","summonerLevel":30,"extra":1}"#,
        )
        .unwrap();
        assert_eq!(s.riot_id(), "Me#EUW");
    }

    #[test]
    fn game_without_participant_is_skipped() {
        let g: RawGame = serde_json::from_str(
            r#"{"gameId":1,"gameCreation":2,"gameDuration":3,"queueId":420,"participants":[]}"#,
        )
        .unwrap();
        assert!(g.into_summary().is_none());
    }
}
