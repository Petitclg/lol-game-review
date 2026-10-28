use lgr_lcu::{LcuApi, LcuClient, LcuError};
use std::thread;
use tiny_http::{Header, Response, Server};

const AUTH: &str = "Basic cmlvdDpwdw=="; // riot:pw

/// Serves the given (path prefix, status, body) routes for exactly one request.
fn serve(routes: Vec<(&'static str, u16, &'static str)>) -> String {
    let server = Server::http("127.0.0.1:0").unwrap();
    let addr = format!("http://{}", server.server_addr().to_ip().unwrap());
    thread::spawn(move || {
        let req = server.recv().unwrap();
        let authorized = req
            .headers()
            .iter()
            .any(|h| h.field.equiv("Authorization") && h.value.as_str() == AUTH);
        let (status, body) = if authorized {
            routes
                .iter()
                .find(|(p, _, _)| req.url().starts_with(p))
                .map_or((404, ""), |(_, s, b)| (*s, *b))
        } else {
            (401, "")
        };
        let json = Header::from_bytes("Content-Type", "application/json").unwrap();
        req.respond(
            Response::from_string(body)
                .with_status_code(status)
                .with_header(json),
        )
        .unwrap();
    });
    addr
}

const SUMMONER: &str = r#"{"puuid":"abc","gameName":"Me","tagLine":"EUW","summonerLevel":42}"#;
const HISTORY: &str = r#"{"games":{"games":[
  {"gameId":10,"gameCreation":1700000000000,"gameDuration":1800,"queueId":420,
   "participants":[{"championId":103,"stats":{"kills":5,"deaths":2,"assists":7,"win":true}}]},
  {"gameId":9,"gameCreation":1699990000000,"gameDuration":1500,"queueId":450,"participants":[]}
]}}"#;

const HISTORY_TWO: &str = r#"{"games":{"games":[
  {"gameId":10,"gameCreation":1,"gameDuration":1,"queueId":420,
   "participants":[{"championId":1,"stats":{"kills":0,"deaths":0,"assists":0,"win":true}}]},
  {"gameId":9,"gameCreation":1,"gameDuration":1,"queueId":420,
   "participants":[{"championId":1,"stats":{"kills":0,"deaths":0,"assists":0,"win":false}}]}
]}}"#;

#[test]
fn fetches_current_summoner() {
    let url = serve(vec![("/lol-summoner/v1/current-summoner", 200, SUMMONER)]);
    let s = LcuClient::with_base_url(&url, "pw")
        .unwrap()
        .current_summoner()
        .unwrap()
        .unwrap();
    assert_eq!(s.riot_id(), "Me#EUW");
    assert_eq!(s.summoner_level, 42);
}

#[test]
fn no_logged_in_account_is_none() {
    let url = serve(vec![("/lol-summoner/v1/current-summoner", 404, "{}")]);
    assert!(LcuClient::with_base_url(&url, "pw")
        .unwrap()
        .current_summoner()
        .unwrap()
        .is_none());
}

#[test]
fn wrong_password_is_reported_as_status() {
    let url = serve(vec![("/lol-summoner/v1/current-summoner", 200, SUMMONER)]);
    let err = LcuClient::with_base_url(&url, "bad")
        .unwrap()
        .current_summoner()
        .unwrap_err();
    assert!(matches!(err, LcuError::Status(401)));
}

#[test]
fn recent_matches_are_summarised_and_empty_games_skipped() {
    let url = serve(vec![("/lol-match-history/", 200, HISTORY)]);
    let m = LcuClient::with_base_url(&url, "pw")
        .unwrap()
        .recent_matches(20)
        .unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!((m[0].game_id, m[0].champion_id), (10, 103));
    assert_eq!((m[0].kills, m[0].deaths, m[0].assists), (5, 2, 7));
    assert!(m[0].win);
}

#[test]
fn recent_matches_never_exceed_requested_count() {
    let url = serve(vec![("/lol-match-history/", 200, HISTORY_TWO)]);
    let m = LcuClient::with_base_url(&url, "pw")
        .unwrap()
        .recent_matches(1)
        .unwrap();
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].game_id, 10);
}

#[test]
fn server_error_is_reported() {
    let url = serve(vec![("/lol-match-history/", 500, "")]);
    let err = LcuClient::with_base_url(&url, "pw")
        .unwrap()
        .recent_matches(5)
        .unwrap_err();
    assert!(matches!(err, LcuError::Status(500)));
}

#[test]
fn connect_reports_not_running_without_lockfile() {
    let err = LcuClient::connect(&[std::path::PathBuf::from(r"Z:\nope\lockfile")])
        .err()
        .unwrap();
    assert!(matches!(err, LcuError::NotRunning));
}
