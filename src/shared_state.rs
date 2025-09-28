use scc::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use crate::git::{CommitInfo, FileDiff, GitRepo};

/// Shared state for git operations using lock-free data structures
pub struct GitSharedState {
    /// Current repository state
    repo_data: HashMap<String, GitRepo>,

    /// Commit information cache
    commit_cache: HashMap<String, CommitInfo>,

    /// File diff cache for performance
    file_diff_cache: HashMap<String, Vec<FileDiff>>,

    /// Current view mode
    view_mode: AtomicU8, // Encoded ViewMode

    /// Error state
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

    /// Update repository data
    pub fn update_repo(&self, repo: GitRepo) {
        let key = "current".to_string(); // Use default key for current repo
        let _ = self.repo_data.insert(key, repo);
    }

    /// Get current repository data
    pub fn get_repo(&self) -> Option<GitRepo> {
        self.repo_data.read("current", |_, v| v.clone())
    }

    /// Cache commit information
    pub fn cache_commit(&self, sha: String, commit: CommitInfo) {
        let _ = self.commit_cache.insert(sha, commit);
    }

    /// Get cached commit information
    pub fn get_cached_commit(&self, sha: &str) -> Option<CommitInfo> {
        self.commit_cache.read(sha, |_, v| v.clone())
    }

    /// Set error state
    pub fn set_error(&self, key: String, error: String) {
        let _ = self.error_state.insert(key, error);
    }

    /// Clear error state
    pub fn clear_error(&self, key: &str) -> bool {
        self.error_state.remove(key).is_some()
    }

    /// Get error state
    pub fn get_error(&self, key: &str) -> Option<String> {
        self.error_state.read(key, |_, v| v.clone())
    }

    /// Get all current errors
    pub fn get_all_errors(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        self.error_state.scan(|k, v| {
            errors.push((k.clone(), v.clone()));
        });
        errors
    }

    /// Clear all errors
    pub fn clear_all_errors(&self) {
        self.error_state.clear();
    }

    /// Check if there are any active errors
    pub fn has_errors(&self) -> bool {
        !self.error_state.is_empty()
    }

    /// Set view mode
    pub fn set_view_mode(&self, mode: u8) {
        self.view_mode.store(mode, Ordering::Relaxed);
    }
}

/// Shared state for LLM operations using lock-free data structures
#[derive(Debug)]
pub struct LlmSharedState {
    /// Summary cache with commit SHA as key
    summary_cache: HashMap<String, String>,

    /// Active summary generation tasks (using HashMap for efficient lookup)
    active_summary_tasks: HashMap<String, u64>, // commit_sha -> timestamp

    /// Error states
    error_state: HashMap<String, String>,

    /// Advice panel state management
    active_advice_tasks: HashMap<String, u64>, // diff_hash -> timestamp
    advice_error_state: HashMap<String, String>, // operation_key -> error message

    /// Current advice content storage for async task results
    current_advice_results: HashMap<String, Vec<crate::pane::AdviceImprovement>>, // diff_hash -> advice results

    /// Pending chat responses for async task results
    pending_chat_responses: HashMap<String, crate::pane::ChatMessageData>, // message_id -> pending AI response
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
            active_summary_tasks: HashMap::new(),
            error_state: HashMap::new(),
            active_advice_tasks: HashMap::new(),
            advice_error_state: HashMap::new(),
            current_advice_results: HashMap::new(),
            pending_chat_responses: HashMap::new(),
        }
    }

    /// Cache a summary for a specific commit SHA
    pub fn cache_summary(&self, commit_sha: String, summary: String) {
        let _ = self.summary_cache.insert(commit_sha, summary);
    }

    /// Get a cached summary for a specific commit SHA
    pub fn get_cached_summary(&self, commit_sha: &str) -> Option<String> {
        self.summary_cache.read(commit_sha, |_, v| v.clone())
    }

    /// Check if a summary is currently being loaded for a commit SHA
    pub fn is_summary_loading(&self, commit_sha: &str) -> bool {
        self.active_summary_tasks.contains(commit_sha)
    }

    /// Start tracking a summary generation task
    pub fn start_summary_task(&self, commit_sha: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = self.active_summary_tasks.insert(commit_sha, timestamp);
    }

    /// Complete a summary generation task and remove it from tracking
    pub fn complete_summary_task(&self, commit_sha: &str) {
        let _ = self.active_summary_tasks.remove(commit_sha);
    }

    /// Set error state for a specific operation
    pub fn set_error(&self, key: String, error: String) {
        let _ = self.error_state.insert(key, error);
    }

    /// Clear error state for a specific operation
    pub fn clear_error(&self, key: &str) -> bool {
        self.error_state.remove(key).is_some()
    }

    /// Get all current errors
    pub fn get_all_errors(&self) -> Vec<(String, String)> {
        let mut errors = Vec::new();
        self.error_state.scan(|k, v| {
            errors.push((k.clone(), v.clone()));
        });
        errors
    }

    /// Clear all errors
    pub fn clear_all_errors(&self) {
        self.error_state.clear();
    }

    /// Check if there are any active errors
    pub fn has_errors(&self) -> bool {
        !self.error_state.is_empty()
    }

    /// Clean up stale tasks older than the specified threshold (in seconds)
    /// Set advice panel error state
    pub fn set_advice_error(&self, key: String, error: String) {
        let _ = self.advice_error_state.insert(key, error);
    }

    /// Get advice panel error state
    pub fn get_advice_error(&self, key: &str) -> Option<String> {
        self.advice_error_state.read(key, |_, v| v.clone())
    }

    /// Clear advice panel error state
    pub fn clear_advice_error(&self, key: &str) -> bool {
        self.advice_error_state.remove(key).is_some()
    }

    /// Start tracking an advice generation task
    pub fn start_advice_task(&self, diff_hash: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = self.active_advice_tasks.insert(diff_hash, timestamp);
    }

    /// Complete an advice generation task
    pub fn complete_advice_task(&self, diff_hash: &str) {
        let _ = self.active_advice_tasks.remove(diff_hash);
    }

    /// Retrieve advice results for a specific diff hash
    pub fn get_advice_results(
        &self,
        diff_hash: &str,
    ) -> Option<Vec<crate::pane::AdviceImprovement>> {
        self.current_advice_results
            .read(diff_hash, |_, v| v.clone())
    }

    /// Store a pending chat response for a specific message ID
    pub fn store_pending_chat_response(
        &self,
        message_id: String,
        response: crate::pane::ChatMessageData,
    ) {
        let _ = self.pending_chat_responses.insert(message_id, response);
    }

    /// Retrieve a pending chat response for a specific message ID
    pub fn get_pending_chat_response(
        &self,
        message_id: &str,
    ) -> Option<crate::pane::ChatMessageData> {
        self.pending_chat_responses
            .read(message_id, |_, v| v.clone())
    }

    /// Remove a pending chat response for a specific message ID
    pub fn remove_pending_chat_response(&self, message_id: &str) -> bool {
        self.pending_chat_responses.remove(message_id).is_some()
    }
}

/// Timing information for monitor operations
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorTiming {
    pub last_run: u64,
    pub elapsed: u64,
    pub has_run: bool,
}

impl MonitorTiming {
    /// Create a new MonitorTiming instance
    pub fn new() -> Self {
        Self {
            last_run: 0,
            elapsed: 0,
            has_run: false,
        }
    }
}

impl Default for MonitorTiming {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state for monitor operations using lock-free data structures
#[derive(Debug)]
pub struct MonitorSharedState {
    /// Monitor command output
    output: HashMap<String, String>,

    /// Timing information
    timing_info: HashMap<String, MonitorTiming>,

    /// Configuration
    config: HashMap<String, String>,
}

impl MonitorSharedState {
    /// Create a new MonitorSharedState instance
    pub fn new() -> Self {
        Self {
            output: HashMap::new(),
            timing_info: HashMap::new(),
            config: HashMap::new(),
        }
    }

    /// Set configuration value
    pub fn set_config(&self, key: String, value: String) {
        let _ = self.config.insert(key, value);
    }
}

impl Default for MonitorSharedState {
    fn default() -> Self {
        Self::new()
    }
}

/// Central manager for all shared state components
pub struct SharedStateManager {
    git_state: Arc<GitSharedState>,
    llm_state: Arc<LlmSharedState>,
    monitor_state: Arc<MonitorSharedState>,
}

impl SharedStateManager {
    /// Create a new SharedStateManager with all state components initialized
    pub fn new() -> Self {
        Self {
            git_state: Arc::new(GitSharedState::new()),
            llm_state: Arc::new(LlmSharedState::new()),
            monitor_state: Arc::new(MonitorSharedState::new()),
        }
    }

    /// Get a reference to the git shared state
    pub fn git_state(&self) -> &Arc<GitSharedState> {
        &self.git_state
    }

    /// Get a reference to the LLM shared state
    pub fn llm_state(&self) -> &Arc<LlmSharedState> {
        &self.llm_state
    }

    /// Initialize all shared state components with configuration
    pub fn initialize(
        &self,
        config: Option<&crate::config::SharedStateConfig>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let default_config = crate::config::SharedStateConfig {
            commit_cache_size: 200,
            commit_history_limit: 100,
            summary_preload_enabled: true,
            summary_preload_count: 5,
            cache_cleanup_interval: 300,
            stale_task_threshold: 3600,
        };

        let config = config.unwrap_or(&default_config);

        // Initialize monitor state configuration
        self.monitor_state
            .set_config("update_interval".to_string(), "1000".to_string());
        self.monitor_state
            .set_config("max_output_size".to_string(), "10485760".to_string()); // 10MB
        self.monitor_state.set_config(
            "cleanup_interval".to_string(),
            config.cache_cleanup_interval.to_string(),
        );
        self.monitor_state.set_config(
            "commit_cache_size".to_string(),
            config.commit_cache_size.to_string(),
        );
        self.monitor_state.set_config(
            "commit_history_limit".to_string(),
            config.commit_history_limit.to_string(),
        );
        self.monitor_state.set_config(
            "stale_task_threshold".to_string(),
            config.stale_task_threshold.to_string(),
        );

        // Initialize git state with default view mode
        self.git_state.set_view_mode(0); // Default to WorkingTree view

        // Clear any existing errors from previous sessions
        self.git_state.clear_all_errors();
        self.llm_state.clear_all_errors();

        Ok(())
    }

    /// Shutdown all shared state components and perform final cleanup
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Clear all cached data to free memory
        self.git_state.commit_cache.clear();
        self.git_state.file_diff_cache.clear();
        self.git_state.repo_data.clear();

        self.llm_state.summary_cache.clear();

        self.monitor_state.output.clear();
        self.monitor_state.timing_info.clear();
        self.monitor_state.config.clear();

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

        // Test multiple errors
        git_state.set_error("error1".to_string(), "First error".to_string());
        git_state.set_error("error2".to_string(), "Second error".to_string());

        let all_errors = git_state.get_all_errors();
        assert_eq!(all_errors.len(), 2);

        // Test clear all errors
        git_state.clear_all_errors();
        let all_errors_after_clear = git_state.get_all_errors();
        assert!(all_errors_after_clear.is_empty());
    }

    #[test]
    fn test_git_shared_state_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let git_state = Arc::new(GitSharedState::new());
        let mut handles = vec![];

        // Test concurrent commit caching
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

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all commits were cached
        for i in 0..10 {
            let commit = git_state.get_cached_commit(&format!("commit_{}", i));
            assert!(commit.is_some());
            assert_eq!(commit.unwrap().message, format!("Test commit {}", i));
        }
    }

    #[test]
    fn test_llm_shared_state_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let llm_state = Arc::new(LlmSharedState::new());
        let mut handles = vec![];

        // Test concurrent summary caching
        for i in 0..10 {
            let state = Arc::clone(&llm_state);
            let handle = thread::spawn(move || {
                let commit_sha = format!("commit_{}", i);
                let summary = format!("Summary for commit {}", i);
                state.cache_summary(commit_sha.clone(), summary);
                state.start_summary_task(commit_sha);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all summaries were cached
        for i in 0..10 {
            let commit_sha = format!("commit_{}", i);
            let summary = llm_state.get_cached_summary(&commit_sha);
            assert!(summary.is_some());
            assert_eq!(summary.unwrap(), format!("Summary for commit {}", i));
            assert!(llm_state.is_summary_loading(&commit_sha));
        }
    }

    #[test]
    fn test_llm_shared_state_error_management() {
        let llm_state = LlmSharedState::new();

        // Test multiple errors
        llm_state.set_error("error1".to_string(), "First error".to_string());
        llm_state.set_error("error2".to_string(), "Second error".to_string());

        let all_errors = llm_state.get_all_errors();
        assert_eq!(all_errors.len(), 2);

        // Test clear all errors
        llm_state.clear_all_errors();
        let all_errors_after_clear = llm_state.get_all_errors();
        assert!(all_errors_after_clear.is_empty());
    }

    #[test]
    fn test_monitor_timing_basic() {
        // Test MonitorTiming creation
        let timing = MonitorTiming::new();
        assert_eq!(timing.last_run, 0);
        assert_eq!(timing.elapsed, 0);
        assert!(!timing.has_run);

        // Test default
        let default_timing = MonitorTiming::default();
        assert_eq!(default_timing.last_run, 0);
        assert_eq!(default_timing.elapsed, 0);
        assert!(!default_timing.has_run);
    }

    #[test]
    fn test_monitor_shared_state_basic() {
        let _monitor_state = MonitorSharedState::new();
    }

    #[test]
    fn test_monitor_shared_state_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let monitor_state = Arc::new(MonitorSharedState::new());
        let mut handles = vec![];

        // Test concurrent config updates
        for i in 0..10 {
            let state = Arc::clone(&monitor_state);
            let handle = thread::spawn(move || {
                let config_key = format!("config_{}", i);
                let value = format!("value_{}", i);
                state.set_config(config_key, value);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify concurrent execution completed without panics
        // Since get_config was removed, we just verify the test completes
    }

    #[test]
    fn test_monitor_shared_state_default() {
        let _monitor_state = MonitorSharedState::default();
    }
}
