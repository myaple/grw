use git2::Status;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: Status,
    pub line_strings: Vec<String>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub files_changed: Vec<CommitFileChange>,
}

#[derive(Debug, Clone)]
pub struct CommitFileChange {
    pub path: PathBuf,
    pub status: FileChangeStatus,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone)]
pub enum FileChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    WorkingTree,
    Staged,
    DirtyDirectory,
    LastCommit,
}
