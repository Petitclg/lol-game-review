//! File-based persistence of sessions, recordings and clips.
//!
//! Layout: `<root>/<session-id>/{session.json, recording.mkv, clips.json, clips/*.mp4}`.

use crate::annotations::{
    check_part_id, tag_from_legacy_note, Annotation, AnnotationError, Annotations, MAX_TAGS,
};
use crate::session::{Clip, Session};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Serialises read-modify-write cycles on `annotations.json`: the UI may save two
/// highlights in quick succession.
static ANNOTATIONS_LOCK: Mutex<()> = Mutex::new(());

/// An annotation as older versions wrote it: tags and a free note. The note is read only to
/// be converted into a custom tag.
#[derive(Deserialize)]
struct StoredAnnotation {
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    note: String,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("invalid session id")]
    InvalidId,
    #[error("session not found")]
    NotFound,
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("corrupt data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Annotation(#[from] AnnotationError),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionMeta {
    pub id: String,
    pub started_at_ms: u64,
    pub last_game_time_secs: f64,
    pub has_recording: bool,
    pub clips_ready: bool,
}

/// What the match history says about a game, kept next to the game so that its
/// annotations can still be filtered (champion, year, outcome) once the game has left the
/// history of the client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameSummary {
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

/// Disk usage of the stored games.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub total_bytes: u64,
    pub recordings_bytes: u64,
    pub clips_bytes: u64,
    pub games: usize,
}

/// What the HUD analysis found for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UltimateRecord {
    /// Whether the game interface was located in the recording.
    pub recognized: bool,
    /// Interface scale that was recognised.
    pub hud_scale: Option<f64>,
    /// Game times (secs) of the ultimate casts; empty when the interface was not
    /// recognised.
    pub casts: Vec<f64>,
    /// Game times (secs) of the casts of summoner spell one and two. `None` for analyses
    /// saved before summoner spells were read: they are redone when the video is still
    /// there.
    #[serde(default)]
    pub summoner_casts: Option<[Vec<f64>; 2]>,
    /// Version of the detection that produced this record ([`ANALYSIS_VERSION`]); a
    /// record from an older detection is analysed again when the video is still there.
    #[serde(default)]
    pub analysis_version: u32,
}

impl UltimateRecord {
    /// The casts, in the shape used to build clips.
    #[must_use]
    pub fn to_casts(&self) -> crate::session::Casts {
        crate::session::Casts {
            ultimate: self.casts.clone(),
            summoner: self.summoner_casts.clone().unwrap_or_default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionStore {
    root: PathBuf,
}

/// Ids come from the UI, so they must never be able to escape the store root.
fn validate_id(id: &str) -> Result<(), StoreError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if ok {
        Ok(())
    } else {
        Err(StoreError::InvalidId)
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

impl SessionStore {
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn session_dir(&self, id: &str) -> Result<PathBuf, StoreError> {
        validate_id(id)?;
        Ok(self.root.join(id))
    }

    pub fn recording_path(&self, id: &str) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join("recording.mkv"))
    }

    pub fn clips_dir(&self, id: &str) -> Result<PathBuf, StoreError> {
        Ok(self.session_dir(id)?.join("clips"))
    }

    /// Creates the session directory (and clips directory) if needed.
    pub fn prepare(&self, id: &str) -> Result<PathBuf, StoreError> {
        let dir = self.session_dir(id)?;
        fs::create_dir_all(dir.join("clips"))?;
        Ok(dir)
    }

    pub fn save_session(&self, session: &Session) -> Result<(), StoreError> {
        let dir = self.prepare(&session.id)?;
        write_atomic(&dir.join("session.json"), &serde_json::to_vec(session)?)?;
        Ok(())
    }

    pub fn load_session(&self, id: &str) -> Result<Session, StoreError> {
        let path = self.session_dir(id)?.join("session.json");
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(StoreError::NotFound),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_clips(&self, id: &str, clips: &[Clip]) -> Result<(), StoreError> {
        let dir = self.prepare(id)?;
        write_atomic(&dir.join("clips.json"), &serde_json::to_vec(clips)?)?;
        Ok(())
    }

    /// Clips of a session; empty when none were generated yet.
    pub fn load_clips(&self, id: &str) -> Result<Vec<Clip>, StoreError> {
        let path = self.session_dir(id)?.join("clips.json");
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    /// Outcome of the ultimate detection, or `None` when it has not run yet for this
    /// session. Files written by older versions (a bare list of cast times) are read too.
    pub fn load_ultimates(&self, id: &str) -> Result<Option<UltimateRecord>, StoreError> {
        let path = self.session_dir(id)?.join("ultimates.json");
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if let Ok(record) = serde_json::from_slice::<UltimateRecord>(&bytes) {
            return Ok(Some(record));
        }
        let legacy: Vec<f64> = serde_json::from_slice(&bytes)?;
        Ok(Some(UltimateRecord {
            recognized: true,
            hud_scale: None,
            casts: legacy,
            summoner_casts: None,
            analysis_version: 0,
        }))
    }

    pub fn save_ultimates(&self, id: &str, record: &UltimateRecord) -> Result<(), StoreError> {
        let dir = self.prepare(id)?;
        write_atomic(&dir.join("ultimates.json"), &serde_json::to_vec(record)?)?;
        Ok(())
    }

    /// Forgets the ultimate analysis (its cast times depend on the video anchor).
    pub fn delete_ultimates(&self, id: &str) -> Result<(), StoreError> {
        match fs::remove_file(self.session_dir(id)?.join("ultimates.json")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// The annotations of a game; empty when there are none. Free notes written by older
    /// versions become custom tags (and the file is rewritten without them).
    pub fn load_annotations(&self, id: &str) -> Result<Annotations, StoreError> {
        let _guard = ANNOTATIONS_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (all, migrated) = self.read_annotations(id)?;
        if migrated {
            // best effort: the converted form is returned either way
            let _ = self.write_annotations(id, &all);
        }
        Ok(all)
    }

    /// Reads `annotations.json`; the flag tells whether a free note was converted.
    fn read_annotations(&self, id: &str) -> Result<(Annotations, bool), StoreError> {
        let path = self.session_dir(id)?.join("annotations.json");
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok((Annotations::new(), false))
            }
            Err(e) => return Err(e.into()),
        };
        let stored: std::collections::BTreeMap<String, StoredAnnotation> =
            serde_json::from_slice(&bytes)?;
        let mut migrated = false;
        let mut all = Annotations::new();
        for (part_id, old) in stored {
            let mut tags: Vec<String> = Vec::new();
            for tag in old.tags {
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            }
            if let Some(tag) = tag_from_legacy_note(&old.note) {
                migrated = true;
                if !tags.contains(&tag) {
                    tags.push(tag);
                }
            } else if !old.note.is_empty() {
                migrated = true;
            }
            tags.truncate(MAX_TAGS);
            all.insert(part_id, Annotation { tags });
        }
        Ok((all, migrated))
    }

    fn write_annotations(&self, id: &str, all: &Annotations) -> Result<(), StoreError> {
        let dir = self.prepare(id)?;
        write_atomic(&dir.join("annotations.json"), &serde_json::to_vec(all)?)?;
        Ok(())
    }

    /// Records the tags of one highlight; none removes the annotation. Returns the
    /// annotation as kept. Annotations are independent of the clips: regenerating or
    /// deleting the clips leaves them alone.
    pub fn set_annotation(
        &self,
        id: &str,
        part_id: &str,
        tags: &[String],
    ) -> Result<Annotation, StoreError> {
        check_part_id(part_id)?;
        let annotation = Annotation::checked(tags)?;
        let _guard = ANNOTATIONS_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mut all, _) = self.read_annotations(id)?;
        if annotation.is_empty() {
            all.remove(part_id);
        } else {
            all.insert(part_id.to_owned(), annotation.clone());
        }
        self.write_annotations(id, &all)?;
        Ok(annotation)
    }

    /// Deletes the clip files that do not belong to `keep` (clip ids), such as clips that
    /// have since been merged into another one. Returns the ids of the deleted files.
    pub fn delete_other_clip_files(
        &self,
        id: &str,
        keep: &[&str],
    ) -> Result<Vec<String>, StoreError> {
        let entries = match fs::read_dir(self.clips_dir(id)?) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut removed = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let is_mp4 = path.extension().is_some_and(|e| e == "mp4");
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if is_mp4 && !keep.contains(&stem) {
                fs::remove_file(&path)?;
                removed.push(stem.to_owned());
            }
        }
        Ok(removed)
    }

    /// The match-history summary saved with the game, if the game was ever listed.
    pub fn load_game_summary(&self, id: &str) -> Result<Option<GameSummary>, StoreError> {
        match fs::read(self.session_dir(id)?.join("game.json")) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).ok()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Saves the match-history summary of the game (nothing is written when it is
    /// unchanged). The game folder must exist.
    pub fn save_game_summary(&self, id: &str, summary: &GameSummary) -> Result<(), StoreError> {
        if self.load_game_summary(id)?.as_ref() == Some(summary) {
            return Ok(());
        }
        let dir = self.session_dir(id)?;
        if !dir.is_dir() {
            return Err(StoreError::NotFound);
        }
        write_atomic(&dir.join("game.json"), &serde_json::to_vec(summary)?)?;
        Ok(())
    }

    /// Deletes the whole session directory (used for discarded sessions).
    pub fn delete_session(&self, id: &str) -> Result<(), StoreError> {
        let dir = self.session_dir(id)?;
        match fs::remove_dir_all(dir) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Removes the source video only; clips and timeline are kept.
    pub fn delete_recording(&self, id: &str) -> Result<(), StoreError> {
        match fs::remove_file(self.recording_path(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Games whose clips are still to be generated: their timeline is saved (the game is
    /// over), their video exists and no clips were generated. Most recent first. Only file
    /// names are looked at, so this is cheap enough to ask often.
    #[must_use]
    pub fn pending_generation(&self) -> Vec<String> {
        let Ok(entries) = fs::read_dir(&self.root) else {
            return Vec::new();
        };
        let mut pending: Vec<(u64, String)> = entries
            .flatten()
            .filter_map(|entry| {
                let id = entry.file_name().to_str()?.to_owned();
                let dir = entry.path();
                let waiting = dir.join("session.json").is_file()
                    && dir.join("recording.mkv").is_file()
                    && !dir.join("clips.json").is_file();
                // ids are `g<start in ms>`: the number orders games by date
                let started = id
                    .strip_prefix('g')
                    .and_then(|n| n.parse().ok())
                    .unwrap_or(0);
                waiting.then_some((started, id))
            })
            .collect();
        pending.sort_by(|a, b| b.cmp(a));
        pending.into_iter().map(|(_, id)| id).collect()
    }

    /// Lists stored sessions, most recent first. Unreadable sessions are skipped.
    pub fn list(&self) -> Result<Vec<SessionMeta>, StoreError> {
        let mut out = Vec::new();
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(out),
            Err(e) => return Err(e.into()),
        };
        for entry in entries.flatten() {
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(session) = self.load_session(&id) else {
                continue;
            };
            let dir = entry.path();
            out.push(SessionMeta {
                id,
                started_at_ms: session.started_at_ms,
                last_game_time_secs: session.last_game_time_secs,
                has_recording: dir.join("recording.mkv").is_file(),
                clips_ready: dir.join("clips.json").is_file(),
            });
        }
        out.sort_by_key(|m| std::cmp::Reverse(m.started_at_ms));
        Ok(out)
    }

    /// Folders that never received a timeline (a crash left them) and were created more
    /// than `min_age` ago. A game in progress has no timeline either, so the age must
    /// comfortably exceed the longest possible game.
    pub fn stale_orphans(&self, min_age: std::time::Duration) -> Result<Vec<String>, StoreError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if !dir.is_dir() || dir.join("session.json").is_file() || validate_id(&id).is_err() {
                continue;
            }
            let created = entry
                .metadata()
                .and_then(|m| m.created().or_else(|_| m.modified()));
            let old_enough = created
                .ok()
                .and_then(|t| t.elapsed().ok())
                .is_some_and(|age| age >= min_age);
            if old_enough {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Size of the source video, `0` when absent.
    pub fn recording_bytes(&self, id: &str) -> Result<u64, StoreError> {
        Ok(fs::metadata(self.recording_path(id)?).map_or(0, |m| m.len()))
    }

    /// Size of the generated clips (files and index), `0` when none.
    pub fn clips_bytes(&self, id: &str) -> Result<u64, StoreError> {
        let dir = self.session_dir(id)?;
        let index = fs::metadata(dir.join("clips.json")).map_or(0, |m| m.len());
        Ok(dir_size(&dir.join("clips")) + index)
    }

    /// Deletes the generated clips (files, index and compression bookkeeping);
    /// the timeline and the source video are kept.
    pub fn delete_clips(&self, id: &str) -> Result<(), StoreError> {
        let dir = self.session_dir(id)?;
        for path in [dir.join("clips.json"), dir.join("compressed.json")] {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                Err(e) => return Err(e.into()),
            }
        }
        match fs::remove_dir_all(dir.join("clips")) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Deletes the generated clip files whose name starts with `prefix` (e.g.
    /// `ultimate-`), so that they are cut again.
    pub fn delete_clip_files(&self, id: &str, prefix: &str) -> Result<(), StoreError> {
        let dir = self.session_dir(id)?.join("clips");
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e.into()),
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with(prefix) {
                fs::remove_file(entry.path())?;
            }
        }
        Ok(())
    }

    /// Ids of the clips whose file was already re-encoded.
    pub fn load_compressed(&self, id: &str) -> Result<Vec<String>, StoreError> {
        let path = self.session_dir(id)?.join("compressed.json");
        match fs::read(path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes).unwrap_or_default()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(e.into()),
        }
    }

    pub fn mark_compressed(&self, id: &str, clip_id: &str) -> Result<(), StoreError> {
        let mut done = self.load_compressed(id)?;
        if !done.iter().any(|c| c == clip_id) {
            done.push(clip_id.to_owned());
        }
        let dir = self.prepare(id)?;
        write_atomic(&dir.join("compressed.json"), &serde_json::to_vec(&done)?)?;
        Ok(())
    }

    /// Forgets that a clip was re-encoded (its file has been replaced).
    pub fn unmark_compressed(&self, id: &str, clip_id: &str) -> Result<(), StoreError> {
        let mut done = self.load_compressed(id)?;
        let before = done.len();
        done.retain(|c| c != clip_id);
        if done.len() != before {
            let dir = self.prepare(id)?;
            write_atomic(&dir.join("compressed.json"), &serde_json::to_vec(&done)?)?;
        }
        Ok(())
    }

    /// Disk usage of all stored sessions, including ones without a timeline
    /// (a recording in progress, or left by a crash).
    pub fn usage(&self) -> Result<StorageUsage, StoreError> {
        let mut usage = StorageUsage::default();
        let entries = match fs::read_dir(&self.root) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(usage),
            Err(e) => return Err(e.into()),
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            usage.total_bytes += dir_size(&dir);
            usage.recordings_bytes +=
                fs::metadata(dir.join("recording.mkv")).map_or(0, |m| m.len());
            usage.clips_bytes += dir_size(&dir.join("clips"));
            if dir.join("session.json").is_file() {
                usage.games += 1;
            }
        }
        Ok(usage)
    }
}

/// Total size of the files under `path`; unreadable entries count as zero.
fn dir_size(path: &Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;

    fn session(id: &str, started: u64) -> Session {
        Session {
            id: id.into(),
            started_at_ms: started,
            player_aliases: vec!["Me".into()],
            video_anchor_secs: 0.0,
            anchor_verified: true,
            recording_file: None,
            last_game_time_secs: 60.0,
            events: vec![],
            snapshots: vec![],
        }
    }

    #[test]
    fn rejects_path_traversal_ids() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        for bad in ["", "..", "../x", "a/b", "a\\b", "C:evil", &"x".repeat(65)] {
            assert!(
                matches!(store.session_dir(bad), Err(StoreError::InvalidId)),
                "{bad}"
            );
        }
        assert!(store.session_dir("1700-1_ok").is_ok());
    }

    #[test]
    fn saves_and_loads_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let s = session("a", 5);
        store.save_session(&s).unwrap();
        assert_eq!(store.load_session("a").unwrap(), s);
    }

    #[test]
    fn missing_session_is_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(matches!(
            store.load_session("zzz"),
            Err(StoreError::NotFound)
        ));
    }

    #[test]
    fn clips_default_to_empty() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(store.load_clips("a").unwrap().is_empty());
    }

    #[test]
    fn list_is_sorted_and_reports_state() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save_session(&session("old", 1)).unwrap();
        store.save_session(&session("new", 2)).unwrap();
        fs::write(store.recording_path("new").unwrap(), b"x").unwrap();
        store.save_clips("old", &[]).unwrap();
        let metas = store.list().unwrap();
        assert_eq!(metas[0].id, "new");
        assert!(metas[0].has_recording && !metas[0].clips_ready);
        assert!(!metas[1].has_recording && metas[1].clips_ready);
    }

    #[test]
    fn list_skips_corrupt_sessions_and_empty_root() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("missing"));
        assert!(store.list().unwrap().is_empty());
        let store = SessionStore::new(dir.path());
        fs::create_dir_all(dir.path().join("bad")).unwrap();
        fs::write(dir.path().join("bad").join("session.json"), b"{").unwrap();
        assert!(store.list().unwrap().is_empty());
    }

    #[test]
    fn ultimates_are_none_until_detected_then_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert_eq!(store.load_ultimates("a").unwrap(), None);
        let rec = UltimateRecord {
            recognized: true,
            hud_scale: Some(1.0),
            casts: vec![482.5, 603.0],
            summoner_casts: Some([vec![120.5], vec![300.0, 640.0]]),
            analysis_version: crate::ultimate::ANALYSIS_VERSION,
        };
        store.save_ultimates("a", &rec).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(rec));
        let none = UltimateRecord {
            recognized: false,
            hud_scale: None,
            casts: vec![],
            summoner_casts: None,
            analysis_version: 0,
        };
        store.save_ultimates("a", &none).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(none));
    }

    #[test]
    fn an_analysis_saved_before_summoner_spells_were_read_loads_without_them() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("a").unwrap();
        fs::write(
            store.session_dir("a").unwrap().join("ultimates.json"),
            br#"{"recognized":true,"hudScale":1.0,"casts":[10.0]}"#,
        )
        .unwrap();
        let rec = store.load_ultimates("a").unwrap().unwrap();
        assert_eq!(rec.summoner_casts, None);
        assert_eq!(rec.analysis_version, 0, "an old record carries no version");
        let casts = rec.to_casts();
        assert_eq!(casts.ultimate, [10.0]);
        assert!(casts.summoner.iter().all(Vec::is_empty));
    }

    #[test]
    fn ultimates_saved_as_a_bare_list_by_older_versions_still_load() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("a").unwrap();
        fs::write(
            store.session_dir("a").unwrap().join("ultimates.json"),
            b"[1.5,2.5]",
        )
        .unwrap();
        let rec = store.load_ultimates("a").unwrap().unwrap();
        assert!(rec.recognized);
        assert_eq!(rec.casts, [1.5, 2.5]);
        assert_eq!(rec.hud_scale, None);
    }

    #[test]
    fn a_game_summary_is_saved_with_the_game_and_only_rewritten_when_it_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save_session(&session("a", 1)).unwrap();
        assert_eq!(store.load_game_summary("a").unwrap(), None);
        let mut summary = GameSummary {
            game_id: 7,
            created_ms: 1_700_000_000_000,
            duration_secs: 1800,
            queue_id: 420,
            champion_id: 103,
            kills: 5,
            deaths: 2,
            assists: 9,
            win: true,
        };
        store.save_game_summary("a", &summary).unwrap();
        assert_eq!(store.load_game_summary("a").unwrap(), Some(summary.clone()));
        let path = store.session_dir("a").unwrap().join("game.json");
        let written = fs::metadata(&path).unwrap().modified().unwrap();
        store.save_game_summary("a", &summary).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), written);
        summary.win = false;
        store.save_game_summary("a", &summary).unwrap();
        assert_eq!(
            store.load_game_summary("a").unwrap().map(|s| s.win),
            Some(false)
        );
    }

    #[test]
    fn a_game_summary_is_not_created_for_a_missing_game_and_a_corrupt_one_reads_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        let summary = GameSummary {
            game_id: 1,
            created_ms: 0,
            duration_secs: 0,
            queue_id: 0,
            champion_id: 0,
            kills: 0,
            deaths: 0,
            assists: 0,
            win: false,
        };
        assert!(matches!(
            store.save_game_summary("nope", &summary),
            Err(StoreError::NotFound)
        ));
        assert!(!dir.path().join("nope").exists());
        store.prepare("a").unwrap();
        fs::write(store.session_dir("a").unwrap().join("game.json"), b"{").unwrap();
        assert_eq!(store.load_game_summary("a").unwrap(), None);
        assert!(matches!(
            store.load_game_summary("../a"),
            Err(StoreError::InvalidId)
        ));
    }

    #[test]
    fn annotations_are_kept_per_highlight_and_an_empty_one_removes_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(store.load_annotations("a").unwrap().is_empty());
        let tags = vec!["face_check".to_owned(), "custom:  mal   placé ".to_owned()];
        let kept = store.set_annotation("a", "death-160.0", &tags).unwrap();
        assert_eq!(kept.tags, ["face_check", "custom:mal placé"]);
        store
            .set_annotation("a", "kill-150.0", &["good_trade".to_owned()])
            .unwrap();
        let all = store.load_annotations("a").unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all["death-160.0"].tags, kept.tags);
        store.set_annotation("a", "death-160.0", &[]).unwrap();
        let all = store.load_annotations("a").unwrap();
        assert_eq!(all.keys().collect::<Vec<_>>(), ["kill-150.0"]);
    }

    #[test]
    fn invalid_annotations_are_refused_and_change_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(matches!(
            store.set_annotation("a", "../x", &[]),
            Err(StoreError::Annotation(AnnotationError::InvalidPartId))
        ));
        assert!(matches!(
            store.set_annotation("a", "kill-1.0", &["Bad".to_owned()]),
            Err(StoreError::Annotation(AnnotationError::InvalidTag(_)))
        ));
        assert!(matches!(
            store.set_annotation("a", "kill-1.0", &["custom:".to_owned()]),
            Err(StoreError::Annotation(AnnotationError::InvalidTag(_)))
        ));
        assert!(matches!(
            store.set_annotation("../a", "kill-1.0", &[]),
            Err(StoreError::InvalidId)
        ));
        assert!(store.load_annotations("a").unwrap().is_empty());
    }

    #[test]
    fn annotations_survive_the_deletion_of_the_clips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save_session(&session("a", 1)).unwrap();
        store
            .set_annotation("a", "kill-1.0", &["custom:à garder".to_owned()])
            .unwrap();
        store.save_clips("a", &[]).unwrap();
        store.delete_clips("a").unwrap();
        assert_eq!(store.load_annotations("a").unwrap().len(), 1);
    }

    #[test]
    fn free_notes_of_older_versions_become_custom_tags_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("a").unwrap();
        let file = store.session_dir("a").unwrap().join("annotations.json");
        fs::write(
            &file,
            r#"{"death-160.0":{"tags":["face_check"],"note":"trop avancé"},
                "kill-150.0":{"tags":[],"note":""},
                "kill-200.0":{"tags":["good_trade"],"note":"good_trade"}}"#
                .as_bytes(),
        )
        .unwrap();
        let all = store.load_annotations("a").unwrap();
        assert_eq!(
            all["death-160.0"].tags,
            ["face_check", "custom:trop avancé"]
        );
        assert!(all["kill-150.0"].is_empty(), "nothing to convert");
        assert_eq!(all["kill-200.0"].tags, ["good_trade", "custom:good_trade"]);
        // the file was rewritten without notes: reading again changes nothing
        let rewritten = fs::read_to_string(&file).unwrap();
        assert!(!rewritten.contains("note"), "{rewritten}");
        assert_eq!(store.load_annotations("a").unwrap(), all);
        assert_eq!(fs::read_to_string(&file).unwrap(), rewritten);
    }

    #[test]
    fn a_long_free_note_is_shortened_when_it_becomes_a_tag() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("a").unwrap();
        let note = "j'ai pris le combat sans voir le jungler adverse arriver par la rivière";
        fs::write(
            store.session_dir("a").unwrap().join("annotations.json"),
            format!(r#"{{"death-1.0":{{"tags":[],"note":"{note}"}}}}"#),
        )
        .unwrap();
        let tags = &store.load_annotations("a").unwrap()["death-1.0"].tags;
        assert_eq!(tags.len(), 1);
        let text = tags[0].strip_prefix("custom:").unwrap();
        assert!(text.chars().count() <= 40 && text.ends_with('…'), "{text}");
        assert!(note.starts_with(text.trim_end_matches('…')));
    }

    #[test]
    fn games_to_generate_have_a_timeline_a_video_and_no_clips_most_recent_first() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(SessionStore::new(dir.path().join("none"))
            .pending_generation()
            .is_empty());
        for (id, started) in [("g100", 100), ("g300", 300), ("g200", 200)] {
            store.save_session(&session(id, started)).unwrap();
            fs::write(store.recording_path(id).unwrap(), b"mkv").unwrap();
        }
        assert_eq!(store.pending_generation(), ["g300", "g200", "g100"]);

        store.save_clips("g300", &[]).unwrap(); // clips generated
        fs::remove_file(store.recording_path("g100").unwrap()).unwrap(); // video gone
        assert_eq!(store.pending_generation(), ["g200"]);
    }

    #[test]
    fn a_game_in_progress_or_without_timeline_is_not_pending() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("g500").unwrap(); // being recorded: no timeline yet
        fs::write(store.recording_path("g500").unwrap(), b"mkv").unwrap();
        assert!(store.pending_generation().is_empty());
        store.save_session(&session("g500", 500)).unwrap();
        assert_eq!(store.pending_generation(), ["g500"]);
    }

    #[test]
    fn games_are_ordered_by_the_date_in_their_id_not_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        for id in ["g9", "g10", "odd"] {
            store.save_session(&session(id, 1)).unwrap();
            fs::write(store.recording_path(id).unwrap(), b"mkv").unwrap();
        }
        assert_eq!(store.pending_generation(), ["g10", "g9", "odd"]);
    }

    #[test]
    fn clip_files_that_are_no_longer_wanted_are_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.prepare("a").unwrap();
        let clips = store.clips_dir("a").unwrap();
        for name in [
            "kill-0.mp4",
            "kill-1.mp4",
            "seq-0.mp4",
            "seq-1.new.mp4",
            "notes.txt",
        ] {
            fs::write(clips.join(name), b"x").unwrap();
        }
        let mut removed = store
            .delete_other_clip_files("a", &["kill-0", "seq-0"])
            .unwrap();
        removed.sort();
        assert_eq!(removed, ["kill-1", "seq-1.new"]);
        assert!(clips.join("kill-0.mp4").is_file() && clips.join("seq-0.mp4").is_file());
        assert!(
            clips.join("notes.txt").is_file(),
            "only clip files are touched"
        );
        assert!(!clips.join("kill-1.mp4").exists());
        assert!(store
            .delete_other_clip_files("none", &[])
            .unwrap()
            .is_empty());
    }

    #[test]
    fn sizes_usage_and_deletions() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save_session(&session("a", 1)).unwrap();
        fs::write(store.recording_path("a").unwrap(), vec![0u8; 1000]).unwrap();
        fs::write(
            store.clips_dir("a").unwrap().join("kill-0.mp4"),
            vec![0u8; 300],
        )
        .unwrap();
        fs::write(
            store.clips_dir("a").unwrap().join("kill-1.mp4"),
            vec![0u8; 200],
        )
        .unwrap();
        store.save_clips("a", &[]).unwrap();
        // a session without a timeline (recording in progress) still counts toward the usage
        let orphan = dir.path().join("g99");
        fs::create_dir_all(&orphan).unwrap();
        fs::write(orphan.join("recording.mkv"), vec![0u8; 5000]).unwrap();

        assert_eq!(store.recording_bytes("a").unwrap(), 1000);
        let clips = store.clips_bytes("a").unwrap();
        assert!((500..600).contains(&clips), "clips {clips}");

        let u = store.usage().unwrap();
        assert_eq!(u.recordings_bytes, 6000);
        assert_eq!(u.games, 1);
        assert!(u.total_bytes > 6500);

        store.delete_recording("a").unwrap();
        assert_eq!(store.recording_bytes("a").unwrap(), 0);
        store.delete_clips("a").unwrap();
        assert_eq!(store.clips_bytes("a").unwrap(), 0);
        assert!(store.load_session("a").is_ok(), "the timeline is kept");
        store.delete_clips("a").unwrap();
    }

    #[test]
    fn compressed_clips_are_remembered_once() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(store.load_compressed("a").unwrap().is_empty());
        store.mark_compressed("a", "kill-0").unwrap();
        store.mark_compressed("a", "kill-0").unwrap();
        store.mark_compressed("a", "death-1").unwrap();
        assert_eq!(store.load_compressed("a").unwrap(), ["kill-0", "death-1"]);
        store.delete_clips("a").unwrap();
        assert!(store.load_compressed("a").unwrap().is_empty());
    }

    #[test]
    fn delete_recording_keeps_session() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save_session(&session("a", 1)).unwrap();
        fs::write(store.recording_path("a").unwrap(), b"x").unwrap();
        store.delete_recording("a").unwrap();
        store.delete_recording("a").unwrap();
        assert!(!store.recording_path("a").unwrap().exists());
        assert!(store.load_session("a").is_ok());
    }
}
