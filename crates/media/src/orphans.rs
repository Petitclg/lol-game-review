//! Cleanup of ffmpeg processes left behind by a crashed or force-killed application.
//!
//! Each recording writes `<recording>.pid`; at startup any pid file whose process is
//! still an ffmpeg is an orphan (the single-instance guard ensures no other running
//! app owns it) and is stopped.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Sidecar file holding the pid of the ffmpeg recording `output`.
#[must_use]
pub fn pid_path(output: &Path) -> PathBuf {
    let mut name = output.as_os_str().to_owned();
    name.push(".pid");
    PathBuf::from(name)
}

fn parse_pid(content: &str) -> Option<u32> {
    content.trim().parse().ok()
}

/// Whether `tasklist /FO CSV /NH` output describes an ffmpeg process.
fn is_ffmpeg_listing(tasklist_output: &str) -> bool {
    tasklist_output
        .to_ascii_lowercase()
        .contains("\"ffmpeg.exe\"")
}

/// A command that never opens a window.
fn hidden(program: &str) -> Command {
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn is_ffmpeg_running(pid: u32) -> bool {
    hidden("tasklist")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
        .is_ok_and(|o| is_ffmpeg_listing(&String::from_utf8_lossy(&o.stdout)))
}

fn kill(pid: u32) {
    let _ = hidden("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .output();
}

/// Stops orphaned recorders found under `sessions_root` and removes their pid files.
/// Returns how many processes were stopped.
pub fn reap_orphans(sessions_root: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(sessions_root) else {
        return 0;
    };
    let mut stopped = 0;
    for entry in entries.flatten() {
        let pid_file = pid_path(&entry.path().join("recording.mkv"));
        let Ok(content) = std::fs::read_to_string(&pid_file) else {
            continue;
        };
        if let Some(pid) = parse_pid(&content) {
            if is_ffmpeg_running(pid) {
                kill(pid);
                stopped += 1;
            }
        }
        let _ = std::fs::remove_file(pid_file);
    }
    stopped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_file_sits_next_to_the_recording() {
        assert_eq!(
            pid_path(Path::new(r"C:\s\g1\recording.mkv")),
            PathBuf::from(r"C:\s\g1\recording.mkv.pid")
        );
    }

    #[test]
    fn parses_pids_and_rejects_garbage() {
        assert_eq!(parse_pid("1234\n"), Some(1234));
        assert_eq!(parse_pid("abc"), None);
        assert_eq!(parse_pid(""), None);
    }

    #[test]
    fn recognizes_only_ffmpeg_listings() {
        assert!(is_ffmpeg_listing(
            "\"ffmpeg.exe\",\"39928\",\"Console\",\"1\",\"120 Ko\"\r\n"
        ));
        assert!(!is_ffmpeg_listing(
            "\"chrome.exe\",\"39928\",\"Console\",\"1\",\"1 Ko\""
        ));
        assert!(!is_ffmpeg_listing(
            "INFO: No tasks are running which match the specified criteria."
        ));
    }

    #[test]
    fn stale_pid_files_are_removed_without_killing_anything() {
        let dir = tempfile::tempdir().unwrap();
        let session = dir.path().join("g1");
        std::fs::create_dir_all(&session).unwrap();
        let pid_file = session.join("recording.mkv.pid");
        // pid 4294967000 cannot belong to a running process
        std::fs::write(&pid_file, "4294967000").unwrap();
        assert_eq!(reap_orphans(dir.path()), 0);
        assert!(!pid_file.exists());
    }

    #[test]
    fn missing_root_is_fine() {
        assert_eq!(reap_orphans(Path::new(r"Z:\does\not\exist")), 0);
    }
}
