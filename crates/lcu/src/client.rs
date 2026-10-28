//! Blocking HTTP client for the LCU API.

use crate::lockfile::{find_lockfile, Lockfile, LockfileError};
use crate::models::{MatchHistory, MatchSummary, Summoner};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum LcuError {
    #[error("league client is not running")]
    NotRunning,
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("cannot read lockfile: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Lockfile(#[from] LockfileError),
    #[error("unexpected status {0}")]
    Status(u16),
}

/// Abstraction used by the application so it can be tested without a client.
pub trait LcuApi {
    /// The logged-in account, or `None` when nobody is signed in.
    fn current_summoner(&self) -> Result<Option<Summoner>, LcuError>;
    /// Most recent games first.
    fn recent_matches(&self, count: u32) -> Result<Vec<MatchSummary>, LcuError>;
}

pub struct LcuClient {
    base_url: String,
    password: String,
    http: reqwest::blocking::Client,
}

const AUTH_USER: &str = "riot";
const TIMEOUT: Duration = Duration::from_secs(5);

impl LcuClient {
    /// Locates the lockfile among `candidates` and connects to the running client.
    pub fn connect(candidates: &[PathBuf]) -> Result<Self, LcuError> {
        let path = find_lockfile(candidates).ok_or(LcuError::NotRunning)?;
        Self::from_lockfile(&Lockfile::parse(&std::fs::read_to_string(path)?)?)
    }

    /// Connects to the local client described by `lockfile`.
    ///
    /// The LCU serves a self-signed certificate, so validation is disabled; this is
    /// acceptable only because the target is always the loopback address.
    pub fn from_lockfile(lockfile: &Lockfile) -> Result<Self, LcuError> {
        let http = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(TIMEOUT)
            .no_proxy()
            .build()?;
        Ok(Self {
            base_url: format!("{}://127.0.0.1:{}", lockfile.protocol, lockfile.port),
            password: lockfile.password.clone(),
            http,
        })
    }

    /// Client with standard certificate validation, for a custom endpoint.
    pub fn with_base_url(base_url: &str, password: &str) -> Result<Self, LcuError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(TIMEOUT)
            .no_proxy()
            .build()?;
        Ok(Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            password: password.to_owned(),
            http,
        })
    }

    fn get(&self, path: &str) -> Result<reqwest::blocking::Response, LcuError> {
        let url = format!("{}{path}", self.base_url);
        Ok(self
            .http
            .get(url)
            .basic_auth(AUTH_USER, Some(&self.password))
            .send()?)
    }
}

impl LcuApi for LcuClient {
    fn current_summoner(&self) -> Result<Option<Summoner>, LcuError> {
        let resp = self.get("/lol-summoner/v1/current-summoner")?;
        match resp.status().as_u16() {
            200 => Ok(Some(resp.json()?)),
            404 => Ok(None),
            s => Err(LcuError::Status(s)),
        }
    }

    fn recent_matches(&self, count: u32) -> Result<Vec<MatchSummary>, LcuError> {
        let path = format!(
            "/lol-match-history/v1/products/lol/current-summoner/matches?begIndex=0&endIndex={count}"
        );
        let resp = self.get(&path)?;
        let status = resp.status().as_u16();
        if status != 200 {
            return Err(LcuError::Status(status));
        }
        let history: MatchHistory = resp.json()?;
        Ok(history
            .games
            .games
            .into_iter()
            .filter_map(crate::models::RawGame::into_summary)
            .take(count as usize)
            .collect())
    }
}
