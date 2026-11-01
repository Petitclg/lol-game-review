//! Free space of the disk that holds the data folder.

use std::path::Path;

/// Bytes available to the application on the disk holding `path` (the nearest existing
/// parent is used when `path` does not exist yet). `None` when it cannot be read: the
/// caller then relies on the disk budget alone.
#[must_use]
pub fn free_bytes(path: &Path) -> Option<u64> {
    let mut dir = path;
    while !dir.exists() {
        dir = dir.parent()?;
    }
    fs4::available_space(dir).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_free_space_of_an_existing_folder_is_known() {
        let dir = tempfile::tempdir().unwrap();
        assert!(free_bytes(dir.path()).is_some_and(|free| free > 0));
    }

    #[test]
    fn a_folder_that_does_not_exist_yet_uses_its_nearest_parent() {
        let dir = tempfile::tempdir().unwrap();
        let later = dir.path().join("not").join("yet");
        assert_eq!(
            free_bytes(&later).is_some(),
            free_bytes(dir.path()).is_some()
        );
    }
}
