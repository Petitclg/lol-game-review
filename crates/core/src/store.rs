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
    /// Game times (secs) of the casts; empty when the interface was not recognised.
    pub casts: Vec<f64>,
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
        }))
    }

    pub fn save_ultimates(&self, id: &str, record: &UltimateRecord) -> Result<(), StoreError> {
        let dir = self.prepare(id)?;
        write_atomic(&dir.join("ultimates.json"), &serde_json::to_vec(record)?)?;
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
        };
        store.save_ultimates("a", &rec).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(rec));
        let none = UltimateRecord {
            recognized: false,
            hud_scale: None,
            casts: vec![],
        };
        store.save_ultimates("a", &none).unwrap();
        assert_eq!(store.load_ultimates("a").unwrap(), Some(none));
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
