//! Application services shared by the desktop shell.

pub mod clipgen;
pub mod games;
pub mod monitor;

pub use clipgen::{generate_clips, ClipCutter, ClipError, FfmpegCutter};
pub use games::{merge_games, GameEntry};
pub use monitor::{ActiveRecording, GameState, Monitor, RecorderFactory, Status};
