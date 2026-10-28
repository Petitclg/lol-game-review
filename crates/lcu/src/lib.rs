//! Client for the local League Client Update (LCU) API.

pub mod client;
pub mod lockfile;
pub mod models;

pub use client::{LcuApi, LcuClient, LcuError};
pub use lockfile::{
    candidates_from_riot_installs, default_lockfile_candidates, discover_lockfile_candidates,
    find_lockfile, Lockfile,
};
pub use models::{MatchSummary, Summoner};
