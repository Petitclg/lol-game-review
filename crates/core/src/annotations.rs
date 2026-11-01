//! The player's tags on the highlights of a game: mistakes, good plays, and tags of their own.
//!
//! An annotation belongs to a clip *part* (see [`crate::session::ClipPart`]), whose id does
//! not change when the clips are regenerated or merged differently. The catalogue of
//! built-in tags and their labels is a presentation matter and lives in the UI; the backend
//! stores tag identifiers and only checks that they are well formed.
//!
//! A tag is either a built-in identifier (`face_check`) or a **custom tag** the player typed,
//! stored as `custom:` followed by its text (`custom:mal placé`) so that the text travels
//! with the annotation, whatever happens to the list of custom tags offered to the player.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// At most this many tags on one highlight.
pub const MAX_TAGS: usize = 24;
/// Longest built-in tag identifier.
pub const MAX_TAG_LEN: usize = 48;
/// Longest text of a custom tag, in characters.
pub const MAX_LABEL_CHARS: usize = 40;
/// Longest highlight identifier.
pub const MAX_PART_ID_LEN: usize = 64;
/// Prefix of the stored form of a custom tag.
pub const CUSTOM_PREFIX: &str = "custom:";

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnnotationError {
    #[error("invalid highlight id")]
    InvalidPartId,
    #[error("invalid tag: {0:?}")]
    InvalidTag(String),
    #[error("too many tags (at most {MAX_TAGS})")]
    TooManyTags,
    #[error("a tag is 1 to {MAX_LABEL_CHARS} characters, without control characters")]
    InvalidLabel,
    #[error("too many custom tags")]
    TooManyCustomTags,
}

/// Tags of one highlight.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    #[serde(default)]
    pub tags: Vec<String>,
}

/// The annotations of a game, by highlight id.
pub type Annotations = BTreeMap<String, Annotation>;

/// The text of a custom tag, cleaned up: surrounding blanks removed and inner runs of
/// blanks collapsed to one space.
pub fn clean_label(text: &str) -> Result<String, AnnotationError> {
    let label = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let ok = !label.is_empty()
        && label.chars().count() <= MAX_LABEL_CHARS
        && !label.chars().any(char::is_control);
    if ok {
        Ok(label)
    } else {
        Err(AnnotationError::InvalidLabel)
    }
}

/// The stored form of a custom tag.
pub fn custom_tag(text: &str) -> Result<String, AnnotationError> {
    Ok(format!("{CUSTOM_PREFIX}{}", clean_label(text)?))
}

/// The custom tag a free note of an older version becomes: its text, shortened with an
/// ellipsis when too long. `None` when the note is empty.
#[must_use]
pub fn tag_from_legacy_note(note: &str) -> Option<String> {
    let flat: String = note
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if flat.is_empty() {
        return None;
    }
    let text = if flat.chars().count() > MAX_LABEL_CHARS {
        let head: String = flat.chars().take(MAX_LABEL_CHARS - 1).collect();
        format!("{}…", head.trim_end())
    } else {
        flat
    };
    Some(format!("{CUSTOM_PREFIX}{text}"))
}

fn check_tag(tag: &str) -> Result<String, AnnotationError> {
    if let Some(text) = tag.strip_prefix(CUSTOM_PREFIX) {
        return match clean_label(text) {
            Ok(label) => Ok(format!("{CUSTOM_PREFIX}{label}")),
            Err(_) => Err(AnnotationError::InvalidTag(tag.to_owned())),
        };
    }
    let ok = !tag.is_empty()
        && tag.len() <= MAX_TAG_LEN
        && tag
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if ok {
        Ok(tag.to_owned())
    } else {
        Err(AnnotationError::InvalidTag(tag.to_owned()))
    }
}

impl Annotation {
    /// `true` when there is nothing to keep.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tags.is_empty()
    }

    /// Checks `tags` and returns them cleaned up: custom tags have their text cleaned,
    /// duplicates are dropped (the first one wins, order is kept).
    pub fn checked(tags: &[String]) -> Result<Self, AnnotationError> {
        let mut clean: Vec<String> = Vec::new();
        for tag in tags {
            let tag = check_tag(tag)?;
            if !clean.contains(&tag) {
                clean.push(tag);
            }
        }
        if clean.len() > MAX_TAGS {
            return Err(AnnotationError::TooManyTags);
        }
        Ok(Self { tags: clean })
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
    fn duplicate_tags_are_dropped_and_the_order_is_kept() {
        let a = Annotation::checked(&tags(&["face_check", "greed", "face_check"])).unwrap();
        assert_eq!(a.tags, ["face_check", "greed"]);
    }

    #[test]
    fn nothing_to_keep_is_empty() {
        assert!(Annotation::checked(&[]).unwrap().is_empty());
        assert!(!Annotation::checked(&tags(&["greed"])).unwrap().is_empty());
    }

    #[test]
    fn malformed_builtin_tags_are_refused() {
        for bad in [
            "",
            "Face-check",
            "face check",
            "../x",
            "é",
            &"a".repeat(MAX_TAG_LEN + 1),
        ] {
            assert_eq!(
                Annotation::checked(&tags(&[bad])),
                Err(AnnotationError::InvalidTag(bad.to_owned())),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_custom_tag_keeps_its_text_cleaned_up() {
        let a =
            Annotation::checked(&tags(&["custom:  mal   placé \t", "custom:mal placé"])).unwrap();
        assert_eq!(a.tags, ["custom:mal placé"], "cleaned, then deduplicated");
        assert!(Annotation::checked(&tags(&["custom:Éclair ⚡ 100 %"])).is_ok());
    }

    #[test]
    fn a_malformed_custom_tag_is_refused() {
        for bad in [
            "custom:".to_owned(),
            "custom:   ".to_owned(),
            "custom:a\u{0}b".to_owned(),
            "custom:a\nb\u{7}".to_owned(),
            format!("custom:{}", "é".repeat(MAX_LABEL_CHARS + 1)),
        ] {
            assert_eq!(
                Annotation::checked(std::slice::from_ref(&bad)),
                Err(AnnotationError::InvalidTag(bad.clone())),
                "{bad:?}"
            );
        }
        assert!(Annotation::checked(&[format!("custom:{}", "é".repeat(MAX_LABEL_CHARS))]).is_ok());
    }

    #[test]
    fn the_number_of_tags_is_limited() {
        let many: Vec<String> = (0..=MAX_TAGS).map(|i| format!("t{i}")).collect();
        assert_eq!(
            Annotation::checked(&many),
            Err(AnnotationError::TooManyTags)
        );
        assert!(Annotation::checked(&many[..MAX_TAGS]).is_ok());
    }

    #[test]
    fn labels_are_cleaned_and_checked() {
        assert_eq!(clean_label("  mal \n placé "), Ok("mal placé".to_owned()));
        assert_eq!(custom_tag("greed 2"), Ok("custom:greed 2".to_owned()));
        for bad in ["", "   ", "a\u{1b}b", &"x".repeat(MAX_LABEL_CHARS + 1)] {
            assert_eq!(
                clean_label(bad),
                Err(AnnotationError::InvalidLabel),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_free_note_of_an_older_version_becomes_a_custom_tag() {
        assert_eq!(
            tag_from_legacy_note("  trop   avancé \n"),
            Some("custom:trop avancé".to_owned())
        );
        assert_eq!(tag_from_legacy_note(""), None);
        assert_eq!(tag_from_legacy_note(" \n\t"), None);
        let long = tag_from_legacy_note(&"mot ".repeat(30)).unwrap();
        let text = long.strip_prefix(CUSTOM_PREFIX).unwrap();
        assert!(text.chars().count() <= MAX_LABEL_CHARS, "{text}");
        assert!(text.ends_with('…'));
        assert!(
            Annotation::checked(&[long]).is_ok(),
            "the result is a valid tag"
        );
        let exact = "é".repeat(MAX_LABEL_CHARS);
        assert_eq!(
            tag_from_legacy_note(&exact),
            Some(format!("custom:{exact}"))
        );
        assert!(Annotation::checked(&[tag_from_legacy_note("a\u{0}b\u{7}c").unwrap()]).is_ok());
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
    fn an_annotation_round_trips_and_tolerates_missing_fields() {
        let a = Annotation::checked(&tags(&["greed", "custom:à revoir"])).unwrap();
        let json = serde_json::to_string(&a).unwrap();
        assert_eq!(json, r#"{"tags":["greed","custom:à revoir"]}"#);
        let back: Annotation = serde_json::from_str("{}").unwrap();
        assert!(back.is_empty());
    }
}
