//! Application services shared by the desktop shell.

pub mod clipgen;
pub mod games;
pub mod monitor;
pub mod schedule;

pub use clipgen::{
    generate_clips, generate_clips_with, load_clips, ClipCutter, ClipError, FfmpegCutter,
    HudUltimateDetector, UltimateDetector,
};
pub use games::{merge_games, GameEntry};
pub use monitor::{ActiveRecording, GameState, Monitor, RecorderFactory, Status};
pub use schedule::{next_check_delay, UpdateOutcome};
