// Library interface for grw (Git Repository Watcher)
// This exposes modules for integration testing

pub mod config;
pub mod git;
pub mod git_worker;
pub mod llm;
pub mod logging;
pub mod monitor;
pub mod pane;
pub mod ui;

// Re-export commonly used types for easier testing
pub use git::{AsyncGitRepo, CommitInfo, CommitFileChange, FileChangeStatus};
pub use ui::{App, Theme};
pub use git_worker::GitWorker;