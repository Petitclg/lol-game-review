//! Background screen recording through an ffmpeg child process.

use crate::ffmpeg::{record_args, silent_command, Encoder};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

pub struct Recording {
    child: Option<Child>,
}

impl Recording {
    /// Starts recording to `output`. Fails if ffmpeg exits right away (no usable
    /// capture device, encoder error, ...).
    pub fn start(ffmpeg: &Path, encoder: Encoder, output: &Path) -> io::Result<Self> {
        let mut cmd = silent_command(ffmpeg);
        cmd.args(record_args(encoder, output)).stdin(Stdio::piped());
        let mut child = cmd.spawn()?;
        std::thread::sleep(Duration::from_millis(400));
        if let Some(status) = child.try_wait()? {
            return Err(io::Error::other(format!("ffmpeg exited early: {status}")));
        }
        Ok(Self { child: Some(child) })
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
        loop {
            if child.try_wait()?.is_some() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                child.kill()?;
                child.wait()?;
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Recording {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}
