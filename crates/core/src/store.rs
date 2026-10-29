//! File-based persistence of sessions, recordings and clips.
//!
//! Layout: `<root>/<session-id>/{session.json, recording.mkv, clips.json, clips/*.mp4}`.

use crate::session::{Clip, Session};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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

fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
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

    /// Game times (secs) of the local player's ultimates, or `None` when detection
    /// has not run yet for this session.
    pub fn load_ultimates(&self, id: &str) -> Result<Option<Vec<f64>>, StoreError> {
        let path = self.session_dir(id)?.join("ultimates.json");
        match fs::read(path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub fn save_ultimates(&self, id: &str, cast_game_secs: &[f64]) -> Result<(), StoreError> {
        let dir = self.prepare(id)?;
        write_atomic(
            &dir.join("ultimates.json"),
            &serde_json::to_vec(cast_game_secs)?,
        )?;
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
        store.save_ultimates("a", &[482.5, 603.0]).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(vec![482.5, 603.0]));
        store.save_ultimates("a", &[]).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(vec![]));
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
