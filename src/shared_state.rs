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

    /// Update repository data
    pub fn update_repo(&self, repo: GitRepo) {
        let key = "current".to_string(); // Use default key for current repo
        let _ = self.repo_data.insert(key, repo);
        self.last_update.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed
        );
    }

    /// Get current repository data
    pub fn get_repo(&self) -> Option<GitRepo> {
        self.repo_data.read("current", |_, v| v.clone())
    }

    /// Get repository data by key (for future multi-repo support)
    pub fn get_repo_by_key(&self, key: &str) -> Option<GitRepo> {
        self.repo_data.read(key, |_, v| v.clone())
    }

    /// Update repository data with custom key
    pub fn update_repo_with_key(&self, key: String, repo: GitRepo) {
        let _ = self.repo_data.insert(key, repo);
        self.last_update.store(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            Ordering::Relaxed
        );
    }

    /// Cache commit information
    pub fn cache_commit(&self, sha: String, commit: CommitInfo) {
        let _ = self.commit_cache.insert(sha, commit);
    }

    /// Get cached commit information
    pub fn get_cached_commit(&self, sha: &str) -> Option<CommitInfo> {
        self.commit_cache.read(sha, |_, v| v.clone())
    }

    /// Cache file diff information
    pub fn cache_file_diff(&self, key: String, diffs: Vec<FileDiff>) {
        let _ = self.file_diff_cache.insert(key, diffs);
    }

    /// Get cached file diff information
    pub fn get_cached_file_diff(&self, key: &str) -> Option<Vec<FileDiff>> {
        self.file_diff_cache.read(key, |_, v| v.clone())
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

    /// Get current view mode
    pub fn get_view_mode(&self) -> u8 {
        self.view_mode.load(Ordering::Relaxed)
    }

    /// Set view mode
    pub fn set_view_mode(&self, mode: u8) {
        self.view_mode.store(mode, Ordering::Relaxed);
    }

    /// Get last update timestamp
    pub fn get_last_update(&self) -> u64 {
        self.last_update.load(Ordering::Relaxed)
    }

    /// Check if data is stale based on given threshold
    pub fn is_stale(&self, threshold_seconds: u64) -> bool {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last_update = self.get_last_update();
        current_time.saturating_sub(last_update) > threshold_seconds
    }
}

/// Shared state for LLM operations using lock-free data structures
pub struct LlmSharedState {
    /// Summary cache with commit SHA as key
    summary_cache: HashMap<String, String>,
    
    /// Advice cache with diff hash as key
    advice_cache: HashMap<String, String>,
    
    /// Active summary generation tasks (using HashMap for efficient lookup)
    active_summary_tasks: HashMap<String, u64>, // commit_sha -> timestamp
    
    /// Active advice generation tasks (using HashMap for efficient lookup)
    active_advice_tasks: HashMap<String, u64>, // task_id -> timestamp
    
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

    /// Start tracking an advice generation task
    pub fn start_advice_task(&self, task_id: String) {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = self.active_advice_tasks.insert(task_id, timestamp);
    }

    /// Complete an advice generation task and remove it from tracking
    pub fn complete_advice_task(&self, task_id: &str) {
        let _ = self.active_advice_tasks.remove(task_id);
    }

    /// Check if an advice task is currently active
    pub fn is_advice_loading(&self, task_id: &str) -> bool {
        self.active_advice_tasks.contains(task_id)
    }

    /// Cache advice content with a specific key
    pub fn cache_advice(&self, key: String, advice: String) {
        let _ = self.advice_cache.insert(key, advice);
    }

    /// Get cached advice for a specific key
    pub fn get_cached_advice(&self, key: &str) -> Option<String> {
        self.advice_cache.read(key, |_, v| v.clone())
    }

    /// Update current advice content
    pub fn update_advice(&self, key: String, advice: String) {
        let _ = self.current_advice.insert(key, advice);
    }

    /// Get current advice content
    pub fn get_current_advice(&self, key: &str) -> Option<String> {
        self.current_advice.read(key, |_, v| v.clone())
    }

    /// Set error state for a specific operation
    pub fn set_error(&self, key: String, error: String) {
        let _ = self.error_state.insert(key, error);
    }

    /// Clear error state for a specific operation
    pub fn clear_error(&self, key: &str) -> bool {
        self.error_state.remove(key).is_some()
    }

    /// Get error state for a specific operation
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

    /// Get count of active summary tasks
    pub fn active_summary_task_count(&self) -> usize {
        self.active_summary_tasks.len()
    }

    /// Get count of active advice tasks
    pub fn active_advice_task_count(&self) -> usize {
        self.active_advice_tasks.len()
    }

    /// Get all active summary tasks with their timestamps
    pub fn get_active_summary_tasks(&self) -> Vec<(String, u64)> {
        let mut tasks = Vec::new();
        self.active_summary_tasks.scan(|k, v| {
            tasks.push((k.clone(), *v));
        });
        tasks
    }

    /// Get all active advice tasks with their timestamps
    pub fn get_active_advice_tasks(&self) -> Vec<(String, u64)> {
        let mut tasks = Vec::new();
        self.active_advice_tasks.scan(|k, v| {
            tasks.push((k.clone(), *v));
        });
        tasks
    }

    /// Clear all active tasks (for cleanup/reset)
    pub fn clear_all_active_tasks(&self) {
        self.active_summary_tasks.clear();
        self.active_advice_tasks.clear();
    }

    /// Clean up stale tasks older than the specified threshold (in seconds)
    pub fn cleanup_stale_tasks(&self, threshold_seconds: u64) {
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Collect stale summary tasks
        let mut stale_summary_tasks = Vec::new();
        self.active_summary_tasks.scan(|k, v| {
            if current_time.saturating_sub(*v) > threshold_seconds {
                stale_summary_tasks.push(k.clone());
            }
        });

        // Remove stale summary tasks
        for task in stale_summary_tasks {
            let _ = self.active_summary_tasks.remove(&task);
        }

        // Collect stale advice tasks
        let mut stale_advice_tasks = Vec::new();
        self.active_advice_tasks.scan(|k, v| {
            if current_time.saturating_sub(*v) > threshold_seconds {
                stale_advice_tasks.push(k.clone());
            }
        });

        // Remove stale advice tasks
        for task in stale_advice_tasks {
            let _ = self.active_advice_tasks.remove(&task);
        }
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

    /// Create a MonitorTiming with current timestamp
    pub fn with_current_time(elapsed: u64) -> Self {
        Self {
            last_run: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            elapsed,
            has_run: true,
        }
    }

    /// Update the timing with new elapsed time and current timestamp
    pub fn update(&mut self, elapsed: u64) {
        self.last_run = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.elapsed = elapsed;
        self.has_run = true;
    }

    /// Check if the timing is stale based on given threshold
    pub fn is_stale(&self, threshold_seconds: u64) -> bool {
        if !self.has_run {
            return true;
        }
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        current_time.saturating_sub(self.last_run) > threshold_seconds
    }
}

impl Default for MonitorTiming {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared state for monitor operations using lock-free data structures
pub struct MonitorSharedState {
    /// Monitor command output
    output: HashMap<String, String>,
    
    /// Timing information
    timing_info: HashMap<String, MonitorTiming>,
    
    /// Configuration
    config: HashMap<String, String>,
    
    /// Error states for monitor operations
    error_state: HashMap<String, String>,
}

impl MonitorSharedState {
    /// Create a new MonitorSharedState instance
    pub fn new() -> Self {
        Self {
            output: HashMap::new(),
            timing_info: HashMap::new(),
            config: HashMap::new(),
            error_state: HashMap::new(),
        }
    }

    /// Update monitor command output
    pub fn update_output(&self, key: String, output: String) {
        let _ = self.output.insert(key, output);
    }

    /// Get monitor command output
    pub fn get_output(&self, key: &str) -> Option<String> {
        self.output.read(key, |_, v| v.clone())
    }

    /// Get all monitor outputs
    pub fn get_all_outputs(&self) -> Vec<(String, String)> {
        let mut outputs = Vec::new();
        self.output.scan(|k, v| {
            outputs.push((k.clone(), v.clone()));
        });
        outputs
    }

    /// Clear monitor output for a specific key
    pub fn clear_output(&self, key: &str) -> bool {
        self.output.remove(key).is_some()
    }

    /// Clear all monitor outputs
    pub fn clear_all_outputs(&self) {
        self.output.clear();
    }

    /// Update timing information for a monitor command
    pub fn update_timing(&self, key: String, timing: MonitorTiming) {
        let _ = self.timing_info.insert(key, timing);
    }

    /// Update timing with elapsed time (convenience method)
    pub fn update_timing_with_elapsed(&self, key: String, elapsed: u64) {
        let timing = MonitorTiming::with_current_time(elapsed);
        let _ = self.timing_info.insert(key, timing);
    }

    /// Get timing information for a monitor command
    pub fn get_timing(&self, key: &str) -> Option<MonitorTiming> {
        self.timing_info.read(key, |_, v| v.clone())
    }

    /// Get all timing information
    pub fn get_all_timing(&self) -> Vec<(String, MonitorTiming)> {
        let mut timings = Vec::new();
        self.timing_info.scan(|k, v| {
            timings.push((k.clone(), v.clone()));
        });
        timings
    }

    /// Clear timing information for a specific key
    pub fn clear_timing(&self, key: &str) -> bool {
        self.timing_info.remove(key).is_some()
    }

    /// Clear all timing information
    pub fn clear_all_timing(&self) {
        self.timing_info.clear();
    }

    /// Set configuration value
    pub fn set_config(&self, key: String, value: String) {
        let _ = self.config.insert(key, value);
    }

    /// Get configuration value
    pub fn get_config(&self, key: &str) -> Option<String> {
        self.config.read(key, |_, v| v.clone())
    }

    /// Get all configuration values
    pub fn get_all_config(&self) -> Vec<(String, String)> {
        let mut configs = Vec::new();
        self.config.scan(|k, v| {
            configs.push((k.clone(), v.clone()));
        });
        configs
    }

    /// Remove configuration value
    pub fn remove_config(&self, key: &str) -> bool {
        self.config.remove(key).is_some()
    }

    /// Clear all configuration
    pub fn clear_all_config(&self) {
        self.config.clear();
    }

    /// Set error state for a monitor operation
    pub fn set_error(&self, key: String, error: String) {
        let _ = self.error_state.insert(key, error);
    }

    /// Clear error state for a monitor operation
    pub fn clear_error(&self, key: &str) -> bool {
        self.error_state.remove(key).is_some()
    }

    /// Get error state for a monitor operation
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

    /// Check if any monitor commands have stale timing information
    pub fn has_stale_timing(&self, threshold_seconds: u64) -> bool {
        let mut has_stale = false;
        self.timing_info.scan(|_, v| {
            if v.is_stale(threshold_seconds) {
                has_stale = true;
            }
        });
        has_stale
    }

    /// Get count of monitor commands with output
    pub fn output_count(&self) -> usize {
        self.output.len()
    }

    /// Get count of monitor commands with timing information
    pub fn timing_count(&self) -> usize {
        self.timing_info.len()
    }

    /// Get count of configuration entries
    pub fn config_count(&self) -> usize {
        self.config.len()
    }

    /// Get count of error entries
    pub fn error_count(&self) -> usize {
        self.error_state.len()
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

        // Test repo operations with new signature
        git_state.update_repo(test_repo.clone());
        let retrieved_repo = git_state.get_repo();
        assert!(retrieved_repo.is_some());
        assert_eq!(retrieved_repo.unwrap().repo_name, "test-repo");

        // Test repo operations with custom key
        git_state.update_repo_with_key("main".to_string(), test_repo.clone());
        let retrieved_repo = git_state.get_repo_by_key("main");
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

        // Test file diff caching
        let test_diffs = vec![FileDiff {
            path: PathBuf::from("test.rs"),
            status: git2::Status::WT_MODIFIED,
            line_strings: vec!["+ added line".to_string()],
            additions: 1,
            deletions: 0,
        }];
        git_state.cache_file_diff("test_diff".to_string(), test_diffs.clone());
        let retrieved_diffs = git_state.get_cached_file_diff("test_diff");
        assert!(retrieved_diffs.is_some());
        assert_eq!(retrieved_diffs.unwrap().len(), 1);

        // Test error handling
        git_state.set_error("test_error".to_string(), "Test error message".to_string());
        let error = git_state.get_error("test_error");
        assert!(error.is_some());
        assert_eq!(error.unwrap(), "Test error message");

        let cleared = git_state.clear_error("test_error");
        assert!(cleared);
        let cleared_error = git_state.get_error("test_error");
        assert!(cleared_error.is_none());

        // Test view mode operations
        git_state.set_view_mode(2);
        assert_eq!(git_state.get_view_mode(), 2);

        // Test timestamp operations
        let initial_time = git_state.get_last_update();
        assert!(initial_time > 0); // Should be set by update_repo call

        // Test staleness check
        assert!(!git_state.is_stale(3600)); // Should not be stale within an hour
        
        // Create a new state to test staleness with no updates
        let fresh_state = GitSharedState::new();
        assert!(fresh_state.is_stale(0)); // Should be stale with 0 threshold since no updates
    }

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
                    author: "Test Author".to_string(),
                    date: "2023-01-01".to_string(),
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
        
        // Test task count
        assert_eq!(llm_state.active_summary_task_count(), 1);

        // Test task completion
        llm_state.complete_summary_task("def456");
        assert!(!llm_state.is_summary_loading("def456"));
        assert_eq!(llm_state.active_summary_task_count(), 0);

        // Test advice caching
        llm_state.cache_advice("diff_hash_1".to_string(), "Cached advice".to_string());
        let cached_advice = llm_state.get_cached_advice("diff_hash_1");
        assert!(cached_advice.is_some());
        assert_eq!(cached_advice.unwrap(), "Cached advice");

        // Test advice management
        llm_state.update_advice("current".to_string(), "Test advice".to_string());
        let advice = llm_state.get_current_advice("current");
        assert!(advice.is_some());
        assert_eq!(advice.unwrap(), "Test advice");

        // Test advice task tracking
        assert!(!llm_state.is_advice_loading("advice_task_1"));
        llm_state.start_advice_task("advice_task_1".to_string());
        assert!(llm_state.is_advice_loading("advice_task_1"));
        assert_eq!(llm_state.active_advice_task_count(), 1);

        // Test advice task completion
        llm_state.complete_advice_task("advice_task_1");
        assert!(!llm_state.is_advice_loading("advice_task_1"));
        assert_eq!(llm_state.active_advice_task_count(), 0);

        // Test error handling
        llm_state.set_error("summary_error".to_string(), "Failed to generate summary".to_string());
        let error = llm_state.get_error("summary_error");
        assert!(error.is_some());
        assert_eq!(error.unwrap(), "Failed to generate summary");

        let cleared = llm_state.clear_error("summary_error");
        assert!(cleared);
        let cleared_error = llm_state.get_error("summary_error");
        assert!(cleared_error.is_none());

        // Test clearing all tasks
        llm_state.start_summary_task("test1".to_string());
        llm_state.start_advice_task("test2".to_string());
        assert_eq!(llm_state.active_summary_task_count(), 1);
        assert_eq!(llm_state.active_advice_task_count(), 1);
        
        llm_state.clear_all_active_tasks();
        assert_eq!(llm_state.active_summary_task_count(), 0);
        assert_eq!(llm_state.active_advice_task_count(), 0);
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

        // Verify task count
        assert_eq!(llm_state.active_summary_task_count(), 10);
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
    fn test_llm_shared_state_cache_operations() {
        let llm_state = LlmSharedState::new();

        // Test summary cache operations
        let commit_sha = "test_commit_123";
        let summary = "This is a test summary for the commit";
        
        // Initially no summary should exist
        assert!(llm_state.get_cached_summary(commit_sha).is_none());
        
        // Cache the summary
        llm_state.cache_summary(commit_sha.to_string(), summary.to_string());
        
        // Verify summary is cached
        let cached_summary = llm_state.get_cached_summary(commit_sha);
        assert!(cached_summary.is_some());
        assert_eq!(cached_summary.unwrap(), summary);

        // Test advice cache operations
        let diff_hash = "diff_hash_456";
        let advice = "This code looks good, consider adding tests";
        
        // Initially no advice should exist
        assert!(llm_state.get_cached_advice(diff_hash).is_none());
        
        // Cache the advice
        llm_state.cache_advice(diff_hash.to_string(), advice.to_string());
        
        // Verify advice is cached
        let cached_advice = llm_state.get_cached_advice(diff_hash);
        assert!(cached_advice.is_some());
        assert_eq!(cached_advice.unwrap(), advice);

        // Test current advice operations
        let advice_key = "current_diff";
        let current_advice = "Current advice for the diff";
        
        llm_state.update_advice(advice_key.to_string(), current_advice.to_string());
        let retrieved_advice = llm_state.get_current_advice(advice_key);
        assert!(retrieved_advice.is_some());
        assert_eq!(retrieved_advice.unwrap(), current_advice);
    }

    #[test]
    fn test_llm_shared_state_task_management() {
        let llm_state = LlmSharedState::new();

        // Test getting active tasks
        llm_state.start_summary_task("commit1".to_string());
        llm_state.start_summary_task("commit2".to_string());
        llm_state.start_advice_task("advice1".to_string());

        let summary_tasks = llm_state.get_active_summary_tasks();
        let advice_tasks = llm_state.get_active_advice_tasks();

        assert_eq!(summary_tasks.len(), 2);
        assert_eq!(advice_tasks.len(), 1);

        // Verify task names are correct
        let summary_task_names: Vec<String> = summary_tasks.iter().map(|(name, _)| name.clone()).collect();
        assert!(summary_task_names.contains(&"commit1".to_string()));
        assert!(summary_task_names.contains(&"commit2".to_string()));

        let advice_task_names: Vec<String> = advice_tasks.iter().map(|(name, _)| name.clone()).collect();
        assert!(advice_task_names.contains(&"advice1".to_string()));

        // Test that cleanup with a large threshold doesn't remove recent tasks
        llm_state.cleanup_stale_tasks(3600); // 1 hour threshold - tasks should not be stale
        assert_eq!(llm_state.active_summary_task_count(), 2);
        assert_eq!(llm_state.active_advice_task_count(), 1);

        // Test manual cleanup by clearing all tasks
        llm_state.clear_all_active_tasks();
        assert_eq!(llm_state.active_summary_task_count(), 0);
        assert_eq!(llm_state.active_advice_task_count(), 0);
    }

    #[test]
    fn test_monitor_timing_operations() {
        // Test MonitorTiming creation
        let timing = MonitorTiming::new();
        assert_eq!(timing.last_run, 0);
        assert_eq!(timing.elapsed, 0);
        assert!(!timing.has_run);

        // Test MonitorTiming with current time
        let timing_with_time = MonitorTiming::with_current_time(250);
        assert!(timing_with_time.last_run > 0);
        assert_eq!(timing_with_time.elapsed, 250);
        assert!(timing_with_time.has_run);

        // Test MonitorTiming update
        let mut timing_mut = MonitorTiming::new();
        timing_mut.update(500);
        assert!(timing_mut.last_run > 0);
        assert_eq!(timing_mut.elapsed, 500);
        assert!(timing_mut.has_run);

        // Test staleness check
        let fresh_timing = MonitorTiming::with_current_time(100);
        assert!(!fresh_timing.is_stale(3600)); // Should not be stale within an hour

        let never_run_timing = MonitorTiming::new();
        assert!(never_run_timing.is_stale(0)); // Should be stale if never run

        // Test default
        let default_timing = MonitorTiming::default();
        assert_eq!(default_timing.last_run, 0);
        assert_eq!(default_timing.elapsed, 0);
        assert!(!default_timing.has_run);
    }

    #[test]
    fn test_monitor_shared_state_basic_operations() {
        let monitor_state = MonitorSharedState::new();

        // Test output management
        monitor_state.update_output("cmd1".to_string(), "Command output".to_string());
        let output = monitor_state.get_output("cmd1");
        assert!(output.is_some());
        assert_eq!(output.unwrap(), "Command output");

        // Test non-existent output
        let no_output = monitor_state.get_output("nonexistent");
        assert!(no_output.is_none());

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

        // Test timing with elapsed convenience method
        monitor_state.update_timing_with_elapsed("cmd2".to_string(), 750);
        let timing2 = monitor_state.get_timing("cmd2");
        assert!(timing2.is_some());
        let timing2_unwrapped = timing2.unwrap();
        assert!(timing2_unwrapped.last_run > 0);
        assert_eq!(timing2_unwrapped.elapsed, 750);
        assert!(timing2_unwrapped.has_run);

        // Test configuration
        monitor_state.set_config("timeout".to_string(), "30".to_string());
        let config_value = monitor_state.get_config("timeout");
        assert!(config_value.is_some());
        assert_eq!(config_value.unwrap(), "30");

        // Test non-existent config
        let no_config = monitor_state.get_config("nonexistent");
        assert!(no_config.is_none());
    }

    #[test]
    fn test_monitor_shared_state_bulk_operations() {
        let monitor_state = MonitorSharedState::new();

        // Add multiple outputs
        monitor_state.update_output("cmd1".to_string(), "Output 1".to_string());
        monitor_state.update_output("cmd2".to_string(), "Output 2".to_string());
        monitor_state.update_output("cmd3".to_string(), "Output 3".to_string());

        // Test get all outputs
        let all_outputs = monitor_state.get_all_outputs();
        assert_eq!(all_outputs.len(), 3);
        assert_eq!(monitor_state.output_count(), 3);

        // Test clear specific output
        let cleared = monitor_state.clear_output("cmd2");
        assert!(cleared);
        assert_eq!(monitor_state.output_count(), 2);
        assert!(monitor_state.get_output("cmd2").is_none());

        // Test clear all outputs
        monitor_state.clear_all_outputs();
        assert_eq!(monitor_state.output_count(), 0);
        let all_outputs_after_clear = monitor_state.get_all_outputs();
        assert!(all_outputs_after_clear.is_empty());

        // Add multiple timing entries
        let timing1 = MonitorTiming::with_current_time(100);
        let timing2 = MonitorTiming::with_current_time(200);
        monitor_state.update_timing("cmd1".to_string(), timing1);
        monitor_state.update_timing("cmd2".to_string(), timing2);

        // Test get all timing
        let all_timing = monitor_state.get_all_timing();
        assert_eq!(all_timing.len(), 2);
        assert_eq!(monitor_state.timing_count(), 2);

        // Test clear specific timing
        let cleared_timing = monitor_state.clear_timing("cmd1");
        assert!(cleared_timing);
        assert_eq!(monitor_state.timing_count(), 1);

        // Test clear all timing
        monitor_state.clear_all_timing();
        assert_eq!(monitor_state.timing_count(), 0);

        // Add multiple config entries
        monitor_state.set_config("timeout".to_string(), "30".to_string());
        monitor_state.set_config("retries".to_string(), "3".to_string());
        monitor_state.set_config("interval".to_string(), "5".to_string());

        // Test get all config
        let all_config = monitor_state.get_all_config();
        assert_eq!(all_config.len(), 3);
        assert_eq!(monitor_state.config_count(), 3);

        // Test remove specific config
        let removed = monitor_state.remove_config("retries");
        assert!(removed);
        assert_eq!(monitor_state.config_count(), 2);
        assert!(monitor_state.get_config("retries").is_none());

        // Test clear all config
        monitor_state.clear_all_config();
        assert_eq!(monitor_state.config_count(), 0);
    }

    #[test]
    fn test_monitor_shared_state_error_management() {
        let monitor_state = MonitorSharedState::new();

        // Test error setting and retrieval
        monitor_state.set_error("cmd1".to_string(), "Command failed".to_string());
        let error = monitor_state.get_error("cmd1");
        assert!(error.is_some());
        assert_eq!(error.unwrap(), "Command failed");

        // Test multiple errors
        monitor_state.set_error("cmd2".to_string(), "Timeout error".to_string());
        monitor_state.set_error("cmd3".to_string(), "Permission denied".to_string());

        let all_errors = monitor_state.get_all_errors();
        assert_eq!(all_errors.len(), 3);
        assert_eq!(monitor_state.error_count(), 3);

        // Test clear specific error
        let cleared = monitor_state.clear_error("cmd2");
        assert!(cleared);
        assert_eq!(monitor_state.error_count(), 2);
        assert!(monitor_state.get_error("cmd2").is_none());

        // Test clear all errors
        monitor_state.clear_all_errors();
        assert_eq!(monitor_state.error_count(), 0);
        let all_errors_after_clear = monitor_state.get_all_errors();
        assert!(all_errors_after_clear.is_empty());

        // Test clearing non-existent error
        let not_cleared = monitor_state.clear_error("nonexistent");
        assert!(!not_cleared);
    }

    #[test]
    fn test_monitor_shared_state_staleness_detection() {
        let monitor_state = MonitorSharedState::new();

        // Add fresh timing
        let fresh_timing = MonitorTiming::with_current_time(100);
        monitor_state.update_timing("fresh_cmd".to_string(), fresh_timing);

        // Add stale timing (simulate old timestamp)
        let mut stale_timing = MonitorTiming::new();
        stale_timing.last_run = 1000000000; // Very old timestamp
        stale_timing.elapsed = 200;
        stale_timing.has_run = true;
        monitor_state.update_timing("stale_cmd".to_string(), stale_timing);

        // Test staleness detection with reasonable threshold
        assert!(monitor_state.has_stale_timing(3600)); // Should detect stale timing

        // Test with very large threshold
        assert!(!monitor_state.has_stale_timing(999999999)); // Should not detect stale timing

        // Test with no timing entries
        let empty_monitor_state = MonitorSharedState::new();
        assert!(!empty_monitor_state.has_stale_timing(0)); // No timing entries, so no stale timing
    }

    #[test]
    fn test_monitor_shared_state_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let monitor_state = Arc::new(MonitorSharedState::new());
        let mut handles = vec![];

        // Test concurrent output updates
        for i in 0..10 {
            let state = Arc::clone(&monitor_state);
            let handle = thread::spawn(move || {
                let cmd_key = format!("cmd_{}", i);
                let output = format!("Output from command {}", i);
                state.update_output(cmd_key, output);
            });
            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }

        // Verify all outputs were stored
        assert_eq!(monitor_state.output_count(), 10);
        for i in 0..10 {
            let cmd_key = format!("cmd_{}", i);
            let output = monitor_state.get_output(&cmd_key);
            assert!(output.is_some());
            assert_eq!(output.unwrap(), format!("Output from command {}", i));
        }

        // Test concurrent timing updates
        let mut timing_handles = vec![];
        for i in 0..5 {
            let state = Arc::clone(&monitor_state);
            let handle = thread::spawn(move || {
                let timing_key = format!("timing_{}", i);
                let elapsed = (i + 1) * 100;
                state.update_timing_with_elapsed(timing_key, elapsed as u64);
            });
            timing_handles.push(handle);
        }

        // Wait for all timing threads to complete
        for handle in timing_handles {
            handle.join().unwrap();
        }

        // Verify all timing entries were stored
        assert_eq!(monitor_state.timing_count(), 5);
        for i in 0..5 {
            let timing_key = format!("timing_{}", i);
            let timing = monitor_state.get_timing(&timing_key);
            assert!(timing.is_some());
            let timing_unwrapped = timing.unwrap();
            assert_eq!(timing_unwrapped.elapsed, ((i + 1) * 100) as u64);
            assert!(timing_unwrapped.has_run);
        }
    }

    #[test]
    fn test_monitor_shared_state_default() {
        let monitor_state = MonitorSharedState::default();
        assert_eq!(monitor_state.output_count(), 0);
        assert_eq!(monitor_state.timing_count(), 0);
        assert_eq!(monitor_state.config_count(), 0);
        assert_eq!(monitor_state.error_count(), 0);
    }
}