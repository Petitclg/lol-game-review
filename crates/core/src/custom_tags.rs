//! The custom tags offered to the player on every highlight, persisted as `custom-tags.json`.
//!
//! This list is only what is *proposed*: the text of a custom tag also travels with every
//! annotation that uses it (see [`crate::annotations`]), so removing a tag from the list
//! leaves the highlights that carry it untouched.

use crate::annotations::{clean_label, AnnotationError};
use crate::store::{write_atomic, StoreError};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// At most this many custom tags are offered.
pub const MAX_CUSTOM_TAGS: usize = 100;

/// Serialises read-modify-write cycles on the file.
static LOCK: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone)]
pub struct CustomTagStore {
    path: PathBuf,
}

/// Two tags are the same tag when they differ only by case ("Greed" and "greed").
fn same(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

impl CustomTagStore {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The custom tags, in the order they were created. A missing or corrupt file gives
    /// none, and entries that are not valid tags are dropped.
    #[must_use]
    pub fn load(&self) -> Vec<String> {
        let Ok(bytes) = fs::read(&self.path) else {
            return Vec::new();
        };
        let raw: Vec<String> = serde_json::from_slice(&bytes).unwrap_or_default();
        let mut out: Vec<String> = Vec::new();
        for label in raw {
            if let Ok(label) = clean_label(&label) {
                if !out.iter().any(|l| same(l, &label)) {
                    out.push(label);
                }
            }
        }
        out.truncate(MAX_CUSTOM_TAGS);
        out
    }

    fn save(&self, labels: &[String]) -> Result<(), StoreError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        write_atomic(&self.path, &serde_json::to_vec(labels)?)?;
        Ok(())
    }

    /// Adds a custom tag. Returns the list and the label to use: the one already in the list
    /// when the tag exists (whatever its case), the cleaned-up text otherwise.
    pub fn add(&self, text: &str) -> Result<(Vec<String>, String), StoreError> {
        let label = clean_label(text)?;
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut labels = self.load();
        if let Some(existing) = labels.iter().find(|l| same(l, &label)) {
            let existing = existing.clone();
            return Ok((labels, existing));
        }
        if labels.len() >= MAX_CUSTOM_TAGS {
            return Err(AnnotationError::TooManyCustomTags.into());
        }
        labels.push(label.clone());
        self.save(&labels)?;
        Ok((labels, label))
    }

    /// Stops offering a custom tag; unknown tags are ignored. Annotations keep theirs.
    pub fn remove(&self, text: &str) -> Result<Vec<String>, StoreError> {
        let _guard = LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut labels = self.load();
        let before = labels.len();
        labels.retain(|l| !same(l, text.trim()));
        if labels.len() != before {
            self.save(&labels)?;
        }
        Ok(labels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> (tempfile::TempDir, CustomTagStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = CustomTagStore::new(dir.path().join("data").join("custom-tags.json"));
        (dir, store)
    }

    #[test]
    fn there_are_no_custom_tags_at_first() {
        let (_d, store) = store();
        assert!(store.load().is_empty());
    }

    #[test]
    fn a_custom_tag_is_added_cleaned_up_and_kept_in_order() {
        let (_d, store) = store();
        let (labels, label) = store.add("  mal   placé ").unwrap();
        assert_eq!(
            (labels, label),
            (vec!["mal placé".to_owned()], "mal placé".to_owned())
        );
        store.add("side lane").unwrap();
        assert_eq!(store.load(), ["mal placé", "side lane"]);
    }

    #[test]
    fn adding_an_existing_tag_changes_nothing_and_returns_the_existing_text() {
        let (_d, store) = store();
        store.add("Mal placé").unwrap();
        let (labels, label) = store.add("  mal PLACÉ").unwrap();
        assert_eq!(labels, ["Mal placé"]);
        assert_eq!(
            label, "Mal placé",
            "the stored text is the one to put on highlights"
        );
    }

    #[test]
    fn an_invalid_tag_is_refused() {
        let (_d, store) = store();
        for bad in ["", "   ", "a\u{7}", &"x".repeat(41)] {
            assert!(store.add(bad).is_err(), "{bad:?}");
        }
        assert!(store.load().is_empty());
    }

    #[test]
    fn the_list_is_limited() {
        let (_d, store) = store();
        for i in 0..MAX_CUSTOM_TAGS {
            store.add(&format!("tag {i}")).unwrap();
        }
        assert!(store.add("one more").is_err());
        assert_eq!(store.load().len(), MAX_CUSTOM_TAGS);
        store.add("tag 3").unwrap();
    }

    #[test]
    fn a_tag_is_removed_whatever_its_case_and_an_unknown_one_is_ignored() {
        let (_d, store) = store();
        store.add("Mal placé").unwrap();
        store.add("side lane").unwrap();
        assert_eq!(store.remove("MAL PLACÉ").unwrap(), ["side lane"]);
        assert_eq!(store.remove("inconnu").unwrap(), ["side lane"]);
        assert_eq!(store.load(), ["side lane"]);
    }

    #[test]
    fn the_list_survives_a_restart() {
        let (_d, store) = store();
        store.add("à revoir").unwrap();
        let again = CustomTagStore::new(store.path.clone());
        assert_eq!(again.load(), ["à revoir"]);
    }

    #[test]
    fn a_corrupt_or_odd_file_gives_a_clean_list() {
        let (_d, store) = store();
        fs::create_dir_all(store.path.parent().unwrap()).unwrap();
        fs::write(&store.path, b"{").unwrap();
        assert!(store.load().is_empty());
        fs::write(
            &store.path,
            r#"["ok","OK","","  ",  "a\u0007","  b  "]"#.as_bytes(),
        )
        .unwrap();
        assert_eq!(store.load(), ["ok", "b"]);
        store.add("c").unwrap();
        assert_eq!(store.load(), ["ok", "b", "c"]);
    }
}
