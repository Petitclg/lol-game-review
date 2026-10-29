//! Silent screen recording and clip cutting through ffmpeg.

pub mod compress;
pub mod cutter;
pub mod ffmpeg;
pub mod hud;
pub mod orphans;
pub mod recorder;

pub use compress::{compress_clip, detect_compress_encoder};
pub use cutter::{cut_clip, CutError};
pub use ffmpeg::{detect_encoder, locate_ffmpeg, CaptureSource, Encoder};
pub use hud::{extract_digit_ratios, SCALES};
pub use orphans::reap_orphans;
pub use recorder::Recording;
