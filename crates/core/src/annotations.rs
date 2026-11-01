//! The player's notes on the highlights of a game: tags (mistakes, good plays) and a note.
//!
//! An annotation belongs to a clip *part* (see [`crate::session::ClipPart`]), whose id does
//! not change when the clips are regenerated or merged differently. The catalogue of tags
//! and their labels is a presentation matter and lives in the UI; the backend stores tag
//! identifiers and only checks that they are well formed.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// At most this many tags on one highlight.
pub const MAX_TAGS: usize = 24;
/// Longest tag identifier.
pub const MAX_TAG_LEN: usize = 48;
/// Longest note, in characters.
pub const MAX_NOTE_CHARS: usize = 2000;
/// Longest highlight identifier.
pub const MAX_PART_ID_LEN: usize = 64;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnnotationError {
    #[error("invalid highlight id")]
    InvalidPartId,
    #[error("invalid tag: {0:?}")]
    InvalidTag(String),
    #[error("too many tags (at most {MAX_TAGS})")]
    TooManyTags,
    #[error("note too long (at most {MAX_NOTE_CHARS} characters)")]
    NoteTooLong,
}

/// Tags and note of one highlight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: String,
}

/// The annotations of a game, by highlight id.
pub type Annotations = BTreeMap<String, Annotation>;

impl Annotation {
    /// `true` when there is nothing to keep.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty() && self.note.is_empty()
    }

    /// Checks `tags` and `note` and returns them cleaned up: duplicate tags are dropped
    /// (the first one wins, order is kept) and the note loses its surrounding blanks.
    pub fn checked(tags: &[String], note: &str) -> Result<Self, AnnotationError> {
        let mut clean: Vec<String> = Vec::new();
        for tag in tags {
            let ok = !tag.is_empty()
                && tag.len() <= MAX_TAG_LEN
                && tag
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
            if !ok {
                return Err(AnnotationError::InvalidTag(tag.clone()));
            }
            if !clean.contains(tag) {
                clean.push(tag.clone());
            }
        }
        if clean.len() > MAX_TAGS {
            return Err(AnnotationError::TooManyTags);
        }
        let note = note.trim();
        if note.chars().count() > MAX_NOTE_CHARS {
            return Err(AnnotationError::NoteTooLong);
        }
        Ok(Self {
            tags: clean,
            note: note.to_owned(),
        })
    }
}

/// Highlight ids come from the UI: letters, digits, `-`, `_` and `.` only (they look like
/// `death-160.0`).
pub fn check_part_id(id: &str) -> Result<(), AnnotationError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_PART_ID_LEN
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'));
    if ok {
        Ok(())
    } else {
        Err(AnnotationError::InvalidPartId)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| (*t).to_owned()).collect()
    }

    #[test]
    fn duplicate_tags_are_dropped_and_the_note_is_trimmed() {
        let a = Annotation::checked(&tags(&["face_check", "greed", "face_check"]), "  oups \n")
            .unwrap();
        assert_eq!(a.tags, ["face_check", "greed"]);
        assert_eq!(a.note, "oups");
    }

    #[test]
    fn nothing_to_keep_is_empty() {
        assert!(Annotation::checked(&[], "   ").unwrap().is_empty());
        assert!(!Annotation::checked(&tags(&["greed"]), "")
            .unwrap()
            .is_empty());
        assert!(!Annotation::checked(&[], "x").unwrap().is_empty());
    }

    #[test]
    fn malformed_tags_are_refused() {
        for bad in [
            "",
            "Face-check",
            "face check",
            "../x",
            "é",
            &"a".repeat(MAX_TAG_LEN + 1),
        ] {
            assert_eq!(
                Annotation::checked(&tags(&[bad]), ""),
                Err(AnnotationError::InvalidTag(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn the_number_of_tags_and_the_length_of_the_note_are_limited() {
        let many: Vec<String> = (0..=MAX_TAGS).map(|i| format!("t{i}")).collect();
        assert_eq!(
            Annotation::checked(&many, ""),
            Err(AnnotationError::TooManyTags)
        );
        assert!(Annotation::checked(&many[..MAX_TAGS], "").is_ok());
        let long = "é".repeat(MAX_NOTE_CHARS + 1);
        assert_eq!(
            Annotation::checked(&[], &long),
            Err(AnnotationError::NoteTooLong)
        );
        assert!(Annotation::checked(&[], &"é".repeat(MAX_NOTE_CHARS)).is_ok());
    }

    #[test]
    fn highlight_ids_cannot_carry_anything_but_a_plain_identifier() {
        assert!(check_part_id("death-160.0").is_ok());
        assert!(check_part_id("key_moment-1234.5").is_ok());
        for bad in [
            "",
            "a b",
            "a/b",
            "a\\b",
            "é",
            &"a".repeat(MAX_PART_ID_LEN + 1),
        ] {
            assert_eq!(
                check_part_id(bad),
                Err(AnnotationError::InvalidPartId),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn an_annotation_round_trips_in_camel_case_and_tolerates_missing_fields() {
        let a = Annotation::checked(&tags(&["greed"]), "n").unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"tags":["greed"],"note":"n"}"#);
        let back: Annotation = serde_json::from_str("{}").unwrap();
        assert!(back.is_empty());
    }
}
