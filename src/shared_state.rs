use scc::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::git::{CommitInfo, FileDiff, GitRepo};

pub struct GitSharedState {
    repo_data: HashMap<String, GitRepo>,
    commit_cache: HashMap<String, CommitInfo>,
    file_diff_cache: HashMap<String, Vec<FileDiff>>,
    view_mode: AtomicU8,
    error_state: HashMap<String, String>,
}

impl Default for GitSharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl GitSharedState {
    pub fn new() -> Self {
        Self {
            repo_data: HashMap::new(),
            commit_cache: HashMap::new(),
            file_diff_cache: HashMap::new(),
            view_mode: AtomicU8::new(0),
            error_state: HashMap::new(),
        }
    }

    pub fn update_repo(&self, repo: GitRepo) {
        use log::trace;
        let key = "current".to_string();
        self.repo_data.upsert(key, repo);
        trace!("Updated repo data in shared state");
    }

    pub fn get_repo(&self) -> Option<GitRepo> {
        self.repo_data.read("current", |_, v| v.clone())
    }

    pub fn cache_commit(&self, sha: String, commit: CommitInfo) {
        self.commit_cache.upsert(sha, commit);
    }

    pub fn get_cached_commit(&self, sha: &str) -> Option<CommitInfo> {
        self.commit_cache.read(sha, |_, v| v.clone())
    }

    pub fn set_error(&self, key: String, error: String) {
        self.error_state.upsert(key, error);
    }

    pub fn clear_error(&self, key: &str) -> bool {
        self.error_state.remove(key).is_some()
    }

    pub fn get_error(&self, key: &str) -> Option<String> {
        self.error_state.read(key, |_, v| v.clone())
    }

    pub fn clear_all_errors(&self) {
        self.error_state.clear();
    }

    pub fn set_view_mode(&self, mode: u8) {
        self.view_mode.store(mode, Ordering::Relaxed);
    }
}

#[derive(Debug)]
pub struct LlmSharedState {
    summary_cache: HashMap<String, String>,
    error_state: HashMap<String, String>,
}

impl Default for LlmSharedState {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmSharedState {
    pub fn new() -> Self {
        Self {
            summary_cache: HashMap::new(),
            error_state: HashMap::new(),
        }
    }

    pub fn clear_all_errors(&self) {
        self.error_state.clear();
    }

    pub fn clear_all_caches(&self) {
        self.summary_cache.clear();
    }
}

pub struct SharedStateManager {
    git_state: Arc<GitSharedState>,
    llm_state: Arc<LlmSharedState>,
}

impl SharedStateManager {
    pub fn new() -> Self {
        Self {
            git_state: Arc::new(GitSharedState::new()),
            llm_state: Arc::new(LlmSharedState::new()),
        }
    }

    pub fn git_state(&self) -> &Arc<GitSharedState> {
        &self.git_state
    }

    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.git_state.set_view_mode(0);
        self.git_state.clear_all_errors();
        self.llm_state.clear_all_errors();
        Ok(())
    }

    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.git_state.commit_cache.clear();
        self.git_state.file_diff_cache.clear();
        self.git_state.repo_data.clear();
        self.llm_state.clear_all_caches();
        Ok(())
    }
}

impl Default for SharedStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_shared_state_error_management() {
        let git_state = GitSharedState::new();

        git_state.set_error("error1".to_string(), "First error".to_string());
        git_state.set_error("error2".to_string(), "Second error".to_string());

        assert!(git_state.get_error("error1").is_some());
        assert!(git_state.get_error("error2").is_some());

        git_state.clear_all_errors();
        assert!(git_state.get_error("error1").is_none());
        assert!(git_state.get_error("error2").is_none());
    }

    #[test]
    fn test_git_shared_state_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let git_state = Arc::new(GitSharedState::new());
        let mut handles = vec![];

        for i in 0..10 {
            let state = Arc::clone(&git_state);
            let handle = thread::spawn(move || {
                let commit = CommitInfo {
                    sha: format!("commit_{}", i),
                    short_sha: format!("commit_{}", i),
                    message: format!("Test commit {}", i),
                    files_changed: vec![],
                };
                state.cache_commit(format!("commit_{}", i), commit);
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        for i in 0..10 {
            let commit = git_state.get_cached_commit(&format!("commit_{}", i));
            assert!(commit.is_some());
            assert_eq!(commit.unwrap().message, format!("Test commit {}", i));
        }
    }
}
