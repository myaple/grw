// Library interface for grw (Git Repository Watcher)
// This exposes modules for integration testing

pub mod config;
pub mod git;
pub mod llm;
pub mod logging;
pub mod monitor;
pub mod pane;
pub mod shared_state;
pub mod ui;

// Re-export commonly used types for easier testing
pub use git::worker::GitWorker;
pub use git::{CommitFileChange, CommitInfo, FileChangeStatus};
pub use shared_state::{
    GitSharedState, LlmSharedState, MonitorSharedState, MonitorTiming, SharedStateManager,
};
pub use ui::{App, Theme};
