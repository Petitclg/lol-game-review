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
}
