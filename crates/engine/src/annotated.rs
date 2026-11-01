//! Annotated highlights across all the games, for review by tag, champion, year, outcome.

use crate::clipgen::load_clips;
use lgr_core::events::Theme;
use lgr_core::store::{GameSummary, SessionStore, StoreError};
use serde::Serialize;

/// A highlight carrying an annotation, with what is needed to filter it and to play it.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedMoment {
    pub session_id: String,
    pub part_id: String,
    pub theme: Theme,
    /// Label of the highlight when its clip is known, empty otherwise.
    pub label: String,
    pub game_time_secs: f64,
    pub tags: Vec<String>,
    pub note: String,
    /// The clip showing the highlight (several highlights may share one merged clip).
    pub clip_id: Option<String>,
    /// Absolute path of the clip file, when it still exists.
    pub file: Option<String>,
    /// Champion, outcome and date; `None` for a game that was never listed.
    pub game: Option<GameSummary>,
}

/// Theme and game time of a highlight id such as `key_moment-1234.5`.
fn parse_part_id(id: &str) -> Option<(Theme, f64)> {
    let (theme, time) = id.split_once('-')?;
    Some((Theme::from_key(theme)?, time.parse().ok()?))
}

/// Every annotated highlight of every game, most recent game first, then in game order.
/// Games without annotations are not read beyond their timeline header.
pub fn list_annotated_moments(store: &SessionStore) -> Result<Vec<AnnotatedMoment>, StoreError> {
    let mut out = Vec::new();
    for meta in store.list()? {
        let annotations = store.load_annotations(&meta.id).unwrap_or_default();
        if annotations.is_empty() {
            continue;
        }
        let game = store.load_game_summary(&meta.id).unwrap_or_default();
        let clips = load_clips(store, &meta.id).unwrap_or_default();
        for (part_id, annotation) in annotations {
            let found = clips.iter().find_map(|clip| {
                clip.parts
                    .iter()
                    .find(|p| p.id == part_id)
                    .map(|part| (clip, part))
            });
            let resolved = match found {
                Some((clip, part)) => Some((
                    part.theme,
                    part.label.clone(),
                    part.game_time_secs,
                    Some(clip.id.clone()),
                    clip.file.clone(),
                )),
                None => parse_part_id(&part_id)
                    .map(|(theme, time)| (theme, String::new(), time, None, None)),
            };
            let Some((theme, label, game_time_secs, clip_id, file)) = resolved else {
                continue;
            };
            out.push(AnnotatedMoment {
                session_id: meta.id.clone(),
                part_id,
                theme,
                label,
                game_time_secs,
                tags: annotation.tags,
                note: annotation.note,
                clip_id,
                file,
                game: game.clone(),
            });
        }
    }
    out.sort_by(|a, b| {
        let created = |m: &AnnotatedMoment| m.game.as_ref().map_or(0, |g| g.created_ms);
        created(b)
            .cmp(&created(a))
            .then_with(|| a.session_id.cmp(&b.session_id))
            .then_with(|| a.game_time_secs.total_cmp(&b.game_time_secs))
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipgen::{generate_clips, ClipCutter};
    use lgr_core::events::RawLiveEvent;
    use lgr_core::session::Session;
    use std::path::Path;

    struct Cutter;

    impl ClipCutter for Cutter {
        fn cut(&self, _: &Path, _: f64, _: f64, out: &Path) -> Result<(), String> {
            std::fs::write(out, b"mp4").map_err(|e| e.to_string())
        }
    }

    fn event(id: u32, t: f64, killer: &str, victim: &str) -> RawLiveEvent {
        RawLiveEvent {
            event_id: id,
            event_name: "ChampionKill".into(),
            event_time: t,
            killer_name: Some(killer.into()),
            victim_name: Some(victim.into()),
            ..RawLiveEvent::default()
        }
    }

    fn add_game(store: &SessionStore, id: &str, started: u64, with_clips: bool) {
        store
            .save_session(&Session {
                id: id.into(),
                started_at_ms: started,
                player_aliases: vec!["Me".into()],
                video_anchor_secs: 0.0,
                anchor_verified: true,
                recording_file: None,
                last_game_time_secs: 500.0,
                events: vec![
                    event(1, 100.0, "Me", "X"),
                    event(2, 110.0, "X", "Me"),
                    event(3, 300.0, "Me", "Y"),
                ],
                snapshots: vec![],
            })
            .unwrap();
        if with_clips {
            std::fs::write(store.recording_path(id).unwrap(), b"mkv").unwrap();
            generate_clips(store, &Cutter, id, |_, _| {}).unwrap();
        }
    }

    fn summary(game_id: u64, created_ms: u64, champion_id: u32, win: bool) -> GameSummary {
        GameSummary {
            game_id,
            created_ms,
            duration_secs: 1800,
            queue_id: 420,
            champion_id,
            kills: 1,
            deaths: 1,
            assists: 1,
            win,
        }
    }

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| (*t).to_owned()).collect()
    }

    #[test]
    fn games_without_annotations_give_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, true);
        assert!(list_annotated_moments(&store).unwrap().is_empty());
        assert!(
            list_annotated_moments(&SessionStore::new(dir.path().join("none")))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn an_annotated_moment_comes_with_its_clip_its_game_and_its_label() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, true);
        store
            .save_game_summary("a", &summary(7, 5_000, 103, false))
            .unwrap();
        // the death shares a merged clip with the kill
        store
            .set_annotation("a", "death-110.0", &tags(&["face_check"]), "trop avancé")
            .unwrap();
        let moments = list_annotated_moments(&store).unwrap();
        assert_eq!(moments.len(), 1);
        let m = &moments[0];
        assert_eq!((m.theme, m.game_time_secs), (Theme::Death, 110.0));
        assert_eq!(m.tags, ["face_check"]);
        assert_eq!(m.note, "trop avancé");
        assert_eq!(m.clip_id.as_deref(), Some("seq-0"));
        assert!(m.file.as_ref().is_some_and(|f| Path::new(f).is_file()));
        assert!(!m.label.is_empty());
        assert_eq!(
            m.game.as_ref().map(|g| (g.champion_id, g.win)),
            Some((103, false))
        );
    }

    #[test]
    fn a_moment_whose_clip_is_gone_is_still_listed_without_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, false);
        store
            .set_annotation("a", "key_moment-1234.5", &[], "ace")
            .unwrap();
        store
            .set_annotation("a", "bogus-1.0", &tags(&["greed"]), "")
            .unwrap();
        store
            .set_annotation("a", "death-oops", &tags(&["greed"]), "")
            .unwrap();
        let moments = list_annotated_moments(&store).unwrap();
        assert_eq!(moments.len(), 1, "ids that are not highlights are ignored");
        let m = &moments[0];
        assert_eq!((m.theme, m.game_time_secs), (Theme::KeyMoment, 1234.5));
        assert_eq!(
            (m.clip_id.as_deref(), m.file.as_deref(), m.label.as_str()),
            (None, None, "")
        );
        assert!(m.game.is_none(), "a game never listed has no summary");
    }

    #[test]
    fn a_deleted_clip_file_leaves_no_path() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "a", 1, true);
        store
            .set_annotation("a", "kill-300.0", &tags(&["good_trade"]), "")
            .unwrap();
        let file = list_annotated_moments(&store).unwrap()[0]
            .file
            .clone()
            .unwrap();
        std::fs::remove_file(file).unwrap();
        let after = list_annotated_moments(&store).unwrap();
        assert_eq!(after[0].file, None);
        assert_eq!(
            after[0].clip_id.as_deref(),
            Some("kill-1"),
            "the clip is still known"
        );
    }

    #[test]
    fn moments_are_sorted_by_game_date_then_game_time() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        add_game(&store, "old", 1, true);
        add_game(&store, "new", 2, true);
        add_game(&store, "unknown", 3, true);
        store
            .save_game_summary("old", &summary(1, 1_000, 1, true))
            .unwrap();
        store
            .save_game_summary("new", &summary(2, 9_000, 2, false))
            .unwrap();
        for id in ["old", "new", "unknown"] {
            store
                .set_annotation(id, "kill-300.0", &tags(&["good_trade"]), "")
                .unwrap();
            store
                .set_annotation(id, "kill-100.0", &tags(&["good_trade"]), "")
                .unwrap();
        }
        let order: Vec<_> = list_annotated_moments(&store)
            .unwrap()
            .iter()
            .map(|m| (m.session_id.clone(), m.game_time_secs))
            .collect();
        let expected: Vec<(String, f64)> = [
            ("new", 100.0),
            ("new", 300.0),
            ("old", 100.0),
            ("old", 300.0),
            ("unknown", 100.0),
            ("unknown", 300.0),
        ]
        .iter()
        .map(|(s, t)| ((*s).to_owned(), *t))
        .collect();
        assert_eq!(order, expected);
    }

    #[test]
    fn part_ids_are_parsed_back_into_theme_and_time() {
        assert_eq!(parse_part_id("death-160.0"), Some((Theme::Death, 160.0)));
        assert_eq!(
            parse_part_id("key_moment-5.5"),
            Some((Theme::KeyMoment, 5.5))
        );
        for bad in ["", "death", "death-", "x-1.0", "death-abc", "-1.0"] {
            assert_eq!(parse_part_id(bad), None, "{bad:?}");
        }
    }
}
