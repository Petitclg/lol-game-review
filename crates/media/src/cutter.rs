//! Lossless clip extraction from a recording.

use crate::ffmpeg::{cut_args, run_with_timeout, silent_command};
use std::io;
use std::path::Path;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum CutError {
    #[error("io error: {0}")]
    Io(#[from] io::Error),
    #[error("ffmpeg failed to cut the clip")]
    Failed,
}

/// Cuts `[start_secs, end_secs]` of `input` into `output` without re-encoding.
/// The cut snaps to the previous keyframe (at most about one second early).
pub fn cut_clip(
    ffmpeg: &Path,
    input: &Path,
    start_secs: f64,
    end_secs: f64,
    output: &Path,
) -> Result<(), CutError> {
    let mut cmd = silent_command(ffmpeg);
    cmd.args(cut_args(input, start_secs, end_secs, output));
    if run_with_timeout(cmd, Duration::from_secs(60))? && output.is_file() {
        Ok(())
    } else {
        let _ = std::fs::remove_file(output);
        Err(CutError::Failed)
    }
}
