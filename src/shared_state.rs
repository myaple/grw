use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use scc::HashMap;

use crate::git::{GitRepo, CommitInfo, FileDiff};

/// Shared state for git operations using lock-free data structures
pub struct GitSharedState {
    /// Current repository state
    repo_data: HashMap<String, GitRepo>,
    
    /// Commit information cache
    commit_cache: HashMap<String, CommitInfo>,
    
    /// File diff cache for performance
    file_diff_cache: HashMap<String, Vec<FileDiff>>,
    
    /// Current view mode and metadata
    view_mode: AtomicU8, // Encoded ViewMode
    last_update: AtomicU64, // Timestamp
    
    /// Error state
    error_state: HashMap<String, String>,
}

impl GitSharedState {
    pub fn new() -> Self {
        Self {
            repo_data: HashMap::new(),
            commit_cache: HashMap::new(),
            file_diff_cache: HashMap::new(),
            view_mode: AtomicU8::new(0),
            last_update: AtomicU64::new(0),
            error_state: HashMap::new(),
        }
    }

    pub fn update_repo(&self, key: String, repo: GitRepo) {
        let _ = self.repo_data.insert(key, repo);
        self.last_update.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed
        );
    }

    pub fn get_repo(&self, key: &str) -> Option<GitRepo> {
        self.repo_data.read(key, |_, v| v.clone())
    }

    pub fn cache_commit(&self, sha: String, commit: CommitInfo) {
        let _ = self.commit_cache.insert(sha, commit);
    }

    pub fn get_cached_commit(&self, sha: &str) -> Option<CommitInfo> {
        self.commit_cache.read(sha, |_, v| v.clone())
    }

    pub fn set_error(&self, key: String, error: String) {
        let _ = self.error_state.insert(key, error);
    }

    pub fn clear_error(&self, key: &str) {
        let _ = self.error_state.remove(key);
    }

    pub fn get_error(&self, key: &str) -> Option<String> {
        self.error_state.read(key, |_, v| v.clone())
    }
}

/// Shared state for LLM operations using lock-free data structures
pub struct LlmSharedState {
    /// Summary cache with commit SHA as key
    summary_cache: HashMap<String, String>,
    
    /// Advice cache with diff hash as key
    advice_cache: HashMap<String, String>,
    
    /// Active summary generation tasks
    active_summary_tasks: HashMap<String, bool>,
    
    /// Active advice generation tasks  
    active_advice_tasks: HashMap<String, bool>,
    
    /// Current advice content
    current_advice: HashMap<String, String>,
    
    /// Error states
    error_state: HashMap<String, String>,
}

impl LlmSharedState {
    pub fn new() -> Self {
        Self {
            summary_cache: HashMap::new(),
            advice_cache: HashMap::new(),
            active_summary_tasks: HashMap::new(),
            active_advice_tasks: HashMap::new(),
            current_advice: HashMap::new(),
            error_state: HashMap::new(),
        }
    }

    pub fn cache_summary(&self, commit_sha: String, summary: String) {
        let _ = self.summary_cache.insert(commit_sha, summary);
    }

    pub fn get_cached_summary(&self, commit_sha: &str) -> Option<String> {
        self.summary_cache.read(commit_sha, |_, v| v.clone())
    }

    pub fn is_summary_loading(&self, commit_sha: &str) -> bool {
        self.active_summary_tasks.contains(commit_sha)
    }

    pub fn start_summary_task(&self, commit_sha: String) {
        let _ = self.active_summary_tasks.insert(commit_sha, true);
    }

    pub fn complete_summary_task(&self, commit_sha: &str) {
        let _ = self.active_summary_tasks.remove(commit_sha);
    }

    pub fn update_advice(&self, key: String, advice: String) {
        let _ = self.current_advice.insert(key, advice);
    }

    pub fn get_current_advice(&self, key: &str) -> Option<String> {
        self.current_advice.read(key, |_, v| v.clone())
    }
}

/// Timing information for monitor operations
#[derive(Clone)]
pub struct MonitorTiming {
    pub last_run: u64,
    pub elapsed: u64,
    pub has_run: bool,
}

/// Shared state for monitor operations using lock-free data structures
pub struct MonitorSharedState {
    /// Monitor command output
    output: HashMap<String, String>,
    
    /// Timing information
    timing_info: HashMap<String, MonitorTiming>,
    
    /// Configuration
    config: HashMap<String, String>,
}

impl MonitorSharedState {
    pub fn new() -> Self {
        Self {
            output: HashMap::new(),
            timing_info: HashMap::new(),
            config: HashMap::new(),
        }
    }

    pub fn update_output(&self, key: String, output: String) {
        let _ = self.output.insert(key, output);
    }

    pub fn get_output(&self, key: &str) -> Option<String> {
        self.output.read(key, |_, v| v.clone())
    }

    pub fn update_timing(&self, key: String, timing: MonitorTiming) {
        let _ = self.timing_info.insert(key, timing);
    }

    pub fn get_timing(&self, key: &str) -> Option<MonitorTiming> {
        self.timing_info.read(key, |_, v| v.clone())
    }

    pub fn set_config(&self, key: String, value: String) {
        let _ = self.config.insert(key, value);
    }

    pub fn get_config(&self, key: &str) -> Option<String> {
        self.config.read(key, |_, v| v.clone())
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

    /// Get a reference to the monitor shared state
    pub fn monitor_state(&self) -> &Arc<MonitorSharedState> {
        &self.monitor_state
    }

    /// Initialize all shared state components with default values
    pub fn initialize(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Initialize any default configuration or state here
        // For now, the components are initialized in their constructors
        Ok(())
    }

    /// Cleanup and shutdown all shared state components
    pub fn shutdown(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Perform any necessary cleanup
        // scc data structures handle their own cleanup automatically
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
    use crate::git::ViewMode;
    use std::path::PathBuf;

    #[test]
    fn test_shared_state_manager_creation() {
        let manager = SharedStateManager::new();
        
        // Verify all components are initialized
        assert!(manager.git_state().repo_data.is_empty());
        assert!(manager.llm_state().summary_cache.is_empty());
        assert!(manager.monitor_state().output.is_empty());
    }

    #[test]
    fn test_shared_state_manager_initialization() {
        let manager = SharedStateManager::new();
        let result = manager.initialize();
        assert!(result.is_ok());
    }

    #[test]
    fn test_shared_state_manager_shutdown() {
        let manager = SharedStateManager::new();
        let result = manager.shutdown();
        assert!(result.is_ok());
    }

    #[test]
    fn test_git_shared_state_operations() {
        let git_state = GitSharedState::new();
        
        // Create a test GitRepo
        let test_repo = GitRepo {
            path: PathBuf::from("/test/repo"),
            changed_files: vec![],
            staged_files: vec![],
            dirty_directory_files: vec![],
            last_commit_files: vec![],
            last_commit_id: Some("abc123".to_string()),
            current_view_mode: ViewMode::WorkingTree,
            repo_name: "test-repo".to_string(),
            branch_name: "main".to_string(),
            commit_info: ("abc123".to_string(), "Test commit".to_string()),
            total_stats: (1, 2, 3),
        };

        // Test repo operations
        git_state.update_repo("main".to_string(), test_repo.clone());
        let retrieved_repo = git_state.get_repo("main");
        assert!(retrieved_repo.is_some());
        assert_eq!(retrieved_repo.unwrap().repo_name, "test-repo");

        // Test commit caching
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };

        git_state.cache_commit("abc123".to_string(), test_commit.clone());
        let retrieved_commit = git_state.get_cached_commit("abc123");
        assert!(retrieved_commit.is_some());
        assert_eq!(retrieved_commit.unwrap().message, "Test commit");

        // Test error handling
        git_state.set_error("test_error".to_string(), "Test error message".to_string());
        let error = git_state.get_error("test_error");
        assert!(error.is_some());
        assert_eq!(error.unwrap(), "Test error message");

        git_state.clear_error("test_error");
        let cleared_error = git_state.get_error("test_error");
        assert!(cleared_error.is_none());
    }

    #[test]
    fn test_llm_shared_state_operations() {
        let llm_state = LlmSharedState::new();

        // Test summary caching
        llm_state.cache_summary("abc123".to_string(), "Test summary".to_string());
        let summary = llm_state.get_cached_summary("abc123");
        assert!(summary.is_some());
        assert_eq!(summary.unwrap(), "Test summary");

        // Test task tracking
        assert!(!llm_state.is_summary_loading("def456"));
        llm_state.start_summary_task("def456".to_string());
        assert!(llm_state.is_summary_loading("def456"));
        llm_state.complete_summary_task("def456");
        assert!(!llm_state.is_summary_loading("def456"));

        // Test advice management
        llm_state.update_advice("current".to_string(), "Test advice".to_string());
        let advice = llm_state.get_current_advice("current");
        assert!(advice.is_some());
        assert_eq!(advice.unwrap(), "Test advice");
    }

    #[test]
    fn test_monitor_shared_state_operations() {
        let monitor_state = MonitorSharedState::new();

        // Test output management
        monitor_state.update_output("cmd1".to_string(), "Command output".to_string());
        let output = monitor_state.get_output("cmd1");
        assert!(output.is_some());
        assert_eq!(output.unwrap(), "Command output");

        // Test timing management
        let timing = MonitorTiming {
            last_run: 1234567890,
            elapsed: 500,
            has_run: true,
        };
        monitor_state.update_timing("cmd1".to_string(), timing.clone());
        let retrieved_timing = monitor_state.get_timing("cmd1");
        assert!(retrieved_timing.is_some());
        let retrieved = retrieved_timing.unwrap();
        assert_eq!(retrieved.last_run, 1234567890);
        assert_eq!(retrieved.elapsed, 500);
        assert!(retrieved.has_run);

        // Test configuration
        monitor_state.set_config("timeout".to_string(), "30".to_string());
        let config_value = monitor_state.get_config("timeout");
        assert!(config_value.is_some());
        assert_eq!(config_value.unwrap(), "30");
    }
}