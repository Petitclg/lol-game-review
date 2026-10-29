//! User settings, persisted as `settings.json` in the data folder.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Disk budgets are expressed in binary gigabytes (1 Go = 1024³ bytes), like the UI.
pub const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;

const MIN_RETENTION_GB: u32 = 1;
const MAX_RETENTION_GB: u32 = 100_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// Delete the source video once every clip has been generated.
    pub delete_source_after_clips: bool,
    /// Re-encode clips to take less space (done while no game is running).
    pub compress_clips: bool,
    /// Disk budget in gigabytes; the oldest games are deleted first. `None` = unlimited.
    pub retention_gb: Option<u32>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            delete_source_after_clips: true,
            compress_clips: true,
            retention_gb: Some(20),
        }
    }
}

impl Settings {
    /// Clamps values to a sane range (a zero budget would delete everything).
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.retention_gb = self
            .retention_gb
            .map(|gb| gb.clamp(MIN_RETENTION_GB, MAX_RETENTION_GB));
        self
    }

    /// The disk budget in bytes, if limited.
    #[must_use]
    pub fn retention_bytes(&self) -> Option<u64> {
        self.retention_gb.map(|gb| u64::from(gb) * BYTES_PER_GB)
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Current settings; defaults when the file is missing or unreadable.
    #[must_use]
    pub fn load(&self) -> Settings {
        fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice::<Settings>(&b).ok())
            .unwrap_or_default()
            .normalized()
    }

    /// Saves the normalised settings and returns them.
    pub fn save(&self, settings: Settings) -> io::Result<Settings> {
        let settings = settings.normalized();
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&settings)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_save_space() {
        let s = Settings::default();
        assert!(s.delete_source_after_clips && s.compress_clips);
        assert_eq!(s.retention_gb, Some(20));
        assert_eq!(s.retention_bytes(), Some(20 * BYTES_PER_GB));
    }

    #[test]
    fn retention_is_clamped_and_unlimited_stays_unlimited() {
        let low = Settings {
            retention_gb: Some(0),
            ..Settings::default()
        }
        .normalized();
        assert_eq!(low.retention_gb, Some(1));
        let high = Settings {
            retention_gb: Some(u32::MAX),
            ..Settings::default()
        }
        .normalized();
        assert_eq!(high.retention_gb, Some(100_000));
        let unlimited = Settings {
            retention_gb: None,
            ..Settings::default()
        }
        .normalized();
        assert_eq!(unlimited.retention_bytes(), None);
    }

    #[test]
    fn missing_or_corrupt_files_give_defaults() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            SettingsStore::new(dir.path().join("settings.json")).load(),
            Settings::default()
        );
        let bad = dir.path().join("bad.json");
        fs::write(&bad, b"{").unwrap();
        assert_eq!(SettingsStore::new(bad).load(), Settings::default());
    }

    #[test]
    fn saved_settings_roundtrip_normalised() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("nested").join("settings.json"));
        let saved = store
            .save(Settings {
                delete_source_after_clips: false,
                compress_clips: false,
                retention_gb: Some(0),
            })
            .unwrap();
        assert_eq!(saved.retention_gb, Some(1));
        assert_eq!(store.load(), saved);
    }

    #[test]
    fn an_explicit_null_means_unlimited_but_a_missing_field_keeps_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s.json");
        fs::write(&path, br#"{"retentionGb":null}"#).unwrap();
        assert_eq!(SettingsStore::new(&path).load().retention_gb, None);
        fs::write(&path, br#"{"compressClips":false}"#).unwrap();
        let s = SettingsStore::new(&path).load();
        assert_eq!((s.compress_clips, s.retention_gb), (false, Some(20)));
    }
}
