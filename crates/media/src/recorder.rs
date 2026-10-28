//! Background screen recording through an ffmpeg child process.

use crate::ffmpeg::{record_args, silent_command, Encoder};
use crate::orphans::pid_path;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub struct Recording {
    child: Option<Child>,
    pid_file: PathBuf,
}

impl Recording {
    /// Starts recording to `output`. Fails if ffmpeg exits right away (no usable
    /// capture device, encoder error, ...). The ffmpeg pid is written next to the
    /// output so that an orphan can be stopped at the next startup.
    pub fn start(ffmpeg: &Path, encoder: Encoder, output: &Path) -> io::Result<Self> {
        let mut cmd = silent_command(ffmpeg);
        cmd.args(record_args(encoder, output)).stdin(Stdio::piped());
        let mut child = cmd.spawn()?;
        let pid_file = pid_path(output);
        let _ = std::fs::write(&pid_file, child.id().to_string());
        std::thread::sleep(Duration::from_millis(400));
        if let Some(status) = child.try_wait()? {
            let _ = std::fs::remove_file(&pid_file);
            return Err(io::Error::other(format!("ffmpeg exited early: {status}")));
        }
        Ok(Self {
            child: Some(child),
            pid_file,
        })
    }

    /// Stops gracefully (ffmpeg finalizes the file on `q`), killing it after 10 s.
    pub fn stop(mut self) -> io::Result<()> {
        self.shutdown()
    }

    fn shutdown(&mut self) -> io::Result<()> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(b"q");
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        let result = loop {
            match child.try_wait() {
                Err(e) => break Err(e),
                Ok(Some(_)) => break Ok(()),
                Ok(None) if Instant::now() >= deadline => {
                    break child.kill().and_then(|()| child.wait()).map(|_| ());
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            }
        };
        let _ = std::fs::remove_file(&self.pid_file);
        result
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
