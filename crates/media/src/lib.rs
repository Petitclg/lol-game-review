//! Silent screen recording and clip cutting through ffmpeg.

pub mod cutter;
pub mod ffmpeg;
pub mod orphans;
pub mod recorder;

pub use cutter::{cut_clip, CutError};
pub use ffmpeg::{detect_encoder, locate_ffmpeg, Encoder};
pub use orphans::reap_orphans;
pub use recorder::Recording;
