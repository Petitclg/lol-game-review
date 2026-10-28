//! Smoke test against a running League client: `cargo run -p lgr-lcu --example whoami`.

use lgr_lcu::{default_lockfile_candidates, LcuApi, LcuClient};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = LcuClient::connect(&default_lockfile_candidates())?;
    match client.current_summoner()? {
        Some(s) => println!("Account: {} (level {})", s.riot_id(), s.summoner_level),
        None => println!("No account signed in"),
    }
    for m in client.recent_matches(5)? {
        println!(
            "game {} champ {} {}/{}/{} {} {}s",
            m.game_id,
            m.champion_id,
            m.kills,
            m.deaths,
            m.assists,
            if m.win { "WIN" } else { "LOSS" },
            m.duration_secs
        );
    }
    Ok(())
}
