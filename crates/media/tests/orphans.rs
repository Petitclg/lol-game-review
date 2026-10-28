//! Spawns a real ffmpeg, abandons it, and checks that `reap_orphans` stops it.

use lgr_media::{locate_ffmpeg, reap_orphans};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[test]
fn an_orphaned_ffmpeg_is_stopped_at_startup() {
    let Some(ffmpeg) = locate_ffmpeg(&[]) else {
        assert!(
            std::env::var_os("LGR_REQUIRE_FFMPEG").is_none(),
            "ffmpeg is required but was not found"
        );
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let session = dir.path().join("g1");
    std::fs::create_dir_all(&session).unwrap();

    // A long-running ffmpeg standing in for a recorder whose app was killed.
    let mut child = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-re",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=size=64x64:rate=5")
        .args(["-t", "120", "-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    std::fs::write(session.join("recording.mkv.pid"), child.id().to_string()).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().unwrap().is_none(),
        "ffmpeg should be running"
    );

    assert_eq!(reap_orphans(dir.path()), 1);

    let deadline = Instant::now() + Duration::from_secs(5);
    while child.try_wait().unwrap().is_none() {
        assert!(Instant::now() < deadline, "orphan was not stopped");
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!session.join("recording.mkv.pid").exists());
}
