//! Parsing and discovery of the League client `lockfile`.

use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Clone, PartialEq, Eq)]
pub struct Lockfile {
    pub pid: u32,
    pub port: u16,
    pub password: String,
    pub protocol: String,
}

// The password must never end up in logs.
impl fmt::Debug for Lockfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Lockfile")
            .field("pid", &self.pid)
            .field("port", &self.port)
            .field("password", &"<redacted>")
            .field("protocol", &self.protocol)
            .finish()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LockfileError {
    #[error("lockfile must have 5 colon-separated fields")]
    Malformed,
    #[error("invalid pid or port in lockfile")]
    InvalidNumber,
}

impl Lockfile {
    /// Parses `name:pid:port:password:protocol`.
    pub fn parse(content: &str) -> Result<Self, LockfileError> {
        let parts: Vec<&str> = content.trim().split(':').collect();
        let [_, pid, port, password, protocol] = parts[..] else {
            return Err(LockfileError::Malformed);
        };
        Ok(Self {
            pid: pid.parse().map_err(|_| LockfileError::InvalidNumber)?,
            port: port.parse().map_err(|_| LockfileError::InvalidNumber)?,
            password: password.to_owned(),
            protocol: protocol.to_owned(),
        })
    }
}

/// Default install locations of the League client, in priority order.
#[must_use]
pub fn default_lockfile_candidates() -> Vec<PathBuf> {
    ["C", "D", "E"]
        .iter()
        .map(|d| PathBuf::from(format!(r"{d}:\Riot Games\League of Legends\lockfile")))
        .collect()
}

/// Lockfile paths derived from Riot's `RiotClientInstalls.json`, whose
/// `associated_client` keys are the install directories of installed games.
#[must_use]
pub fn candidates_from_riot_installs(json: &str) -> Vec<PathBuf> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    value
        .get("associated_client")
        .and_then(serde_json::Value::as_object)
        .map(|m| {
            m.keys()
                .filter(|k| k.to_ascii_lowercase().contains("league of legends"))
                .map(|k| PathBuf::from(k).join("lockfile"))
                .collect()
        })
        .unwrap_or_default()
}

/// Installs recorded by Riot first, then the default locations.
#[must_use]
pub fn discover_lockfile_candidates() -> Vec<PathBuf> {
    let installs = std::env::var_os("PROGRAMDATA")
        .map(|p| {
            PathBuf::from(p)
                .join("Riot Games")
                .join("RiotClientInstalls.json")
        })
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|j| candidates_from_riot_installs(&j))
        .unwrap_or_default();
    let mut all = installs;
    for d in default_lockfile_candidates() {
        if !all.contains(&d) {
            all.push(d);
        }
    }
    all
}

/// Returns the first candidate that exists, i.e. the client is running.
#[must_use]
pub fn find_lockfile(candidates: &[PathBuf]) -> Option<&Path> {
    candidates
        .iter()
        .map(PathBuf::as_path)
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_lockfile() {
        let l = Lockfile::parse("LeagueClient:1234:54321:s3cret:https\n").unwrap();
        assert_eq!(l.pid, 1234);
        assert_eq!(l.port, 54321);
        assert_eq!(l.password, "s3cret");
        assert_eq!(l.protocol, "https");
    }

    #[test]
    fn rejects_wrong_field_count() {
        assert_eq!(Lockfile::parse("a:b:c"), Err(LockfileError::Malformed));
    }

    #[test]
    fn rejects_non_numeric_port() {
        assert_eq!(
            Lockfile::parse("LeagueClient:1:abc:pw:https"),
            Err(LockfileError::InvalidNumber)
        );
    }

    #[test]
    fn debug_never_prints_password() {
        let l = Lockfile::parse("LeagueClient:1:2:topsecret:https").unwrap();
        assert!(!format!("{l:?}").contains("topsecret"));
    }

    #[test]
    fn find_returns_none_when_missing() {
        assert!(find_lockfile(&[PathBuf::from(r"Z:\nope\lockfile")]).is_none());
    }

    #[test]
    fn riot_installs_yield_league_lockfile_only() {
        let json = r#"{"associated_client":{
          "D:/Games/League of Legends/":"D:/Riot/RiotClientServices.exe",
          "C:/Riot Games/VALORANT/":"C:/Riot Games/Riot Client/RiotClientServices.exe"}}"#;
        let c = candidates_from_riot_installs(json);
        assert_eq!(
            c,
            [PathBuf::from("D:/Games/League of Legends/").join("lockfile")]
        );
    }

    #[test]
    fn riot_installs_tolerate_garbage() {
        assert!(candidates_from_riot_installs("nope").is_empty());
        assert!(candidates_from_riot_installs("{}").is_empty());
    }
}
