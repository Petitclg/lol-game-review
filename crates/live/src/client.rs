//! Blocking client for the Live Client Data API (served only while in game).

use crate::models::AllGameData;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum LiveError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unexpected status {0}")]
    Status(u16),
}

/// Source of live game data; abstracted so the monitor can be tested.
pub trait LiveSource {
    /// `Ok(None)` when no game is running (connection refused or not ready).
    fn fetch(&self) -> Result<Option<AllGameData>, LiveError>;
}

pub struct LiveClient {
    url: String,
    http: reqwest::blocking::Client,
}

const DEFAULT_URL: &str = "https://127.0.0.1:2999/liveclientdata/allgamedata";

impl LiveClient {
    /// The API serves a self-signed certificate on the loopback address only.
    pub fn new() -> Result<Self, LiveError> {
        let http = reqwest::blocking::Client::builder()
            .danger_accept_invalid_certs(true)
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()?;
        Ok(Self {
            url: DEFAULT_URL.to_owned(),
            http,
        })
    }

    /// Custom endpoint with standard certificate validation (tests).
    pub fn with_url(url: &str) -> Result<Self, LiveError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .no_proxy()
            .build()?;
        Ok(Self {
            url: url.to_owned(),
            http,
        })
    }
}

impl LiveSource for LiveClient {
    fn fetch(&self) -> Result<Option<AllGameData>, LiveError> {
        let resp = match self.http.get(&self.url).send() {
            Ok(r) => r,
            Err(e) if e.is_connect() || e.is_timeout() => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        match resp.status().as_u16() {
            200 => Ok(resp.json::<AllGameData>().ok()),
            404 => Ok(None),
            s => Err(LiveError::Status(s)),
        }
    }
}
