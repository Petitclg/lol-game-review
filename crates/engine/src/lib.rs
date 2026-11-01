//! Application services shared by the desktop shell.

pub mod annotated;
pub mod autogen;
pub mod clipgen;
pub mod disk;
pub mod games;
pub mod maintenance;
pub mod monitor;
pub mod sandbox;
pub mod schedule;
pub mod storage;

pub use annotated::{list_annotated_moments, AnnotatedMoment};
pub use autogen::{AutoGenerator, InFlight, InFlightToken};
pub use clipgen::{
    generate_clips, generate_clips_with, load_clips, ultimate_status, ClipCutter, ClipError,
    FfmpegCutter, HudUltimateDetector, UltimateDetector, UltimateOutcome, UltimateState,
    UltimateStatus,
};
pub use games::{merge_games, remember_games, GameEntry};
pub use maintenance::{run_maintenance, ClipCompressor, FfmpegCompressor, MaintenanceReport};
pub use monitor::{ActiveRecording, GameState, Monitor, RecorderFactory, Status};
pub use schedule::{next_check_delay, UpdateOutcome};
pub use storage::{
    enforce_retention, finalize_generation, make_room, Reclaimed, RoomReport,
    RECORDING_RESERVE_BYTES,
};
