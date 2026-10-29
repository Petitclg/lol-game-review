//! Sandbox mode: lets a test instance run next to the real installation without ever
//! touching its data or recording the user's games.
//!
//! Enabled with `LGR_SANDBOX=1` (data under the system temp folder) or
//! `LGR_SANDBOX=<directory>`. A sandboxed instance uses its own data and `WebView`
//! folders, never starts the game monitor (so it cannot record), never updates itself
//! and does not take the single-instance lock.

use std::path::{Path, PathBuf};

const DEFAULT_DIR_NAME: &str = "lgr-sandbox";

/// Sandbox directory for the given `LGR_SANDBOX` value, or `None` when not sandboxed.
#[must_use]
pub fn resolve_sandbox(env_value: Option<&str>, temp_dir: &Path) -> Option<PathBuf> {
    match env_value.map(str::trim) {
        None | Some("" | "0") => None,
        Some("1") => Some(temp_dir.join(DEFAULT_DIR_NAME)),
        Some(dir) => Some(PathBuf::from(dir)),
    }
}

/// Sandbox directory of the current process, from the environment.
#[must_use]
pub fn current_sandbox() -> Option<PathBuf> {
    resolve_sandbox(
        std::env::var("LGR_SANDBOX").ok().as_deref(),
        &std::env::temp_dir(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unset_empty_or_zero_means_not_sandboxed() {
        let tmp = Path::new(r"C:\t");
        assert_eq!(resolve_sandbox(None, tmp), None);
        assert_eq!(resolve_sandbox(Some(""), tmp), None);
        assert_eq!(resolve_sandbox(Some("  "), tmp), None);
        assert_eq!(resolve_sandbox(Some("0"), tmp), None);
    }

    #[test]
    fn one_selects_a_folder_under_temp() {
        assert_eq!(
            resolve_sandbox(Some("1"), Path::new(r"C:\t")),
            Some(PathBuf::from(r"C:\t").join("lgr-sandbox"))
        );
    }

    #[test]
    fn any_other_value_is_the_directory() {
        assert_eq!(
            resolve_sandbox(Some(r"D:\work\sb"), Path::new(r"C:\t")),
            Some(PathBuf::from(r"D:\work\sb"))
        );
    }
}
