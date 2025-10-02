pub mod operations;
pub mod worker;

// Re-export worker to maintain the same public API
pub use worker::GitWorker;

use crate::shared_state::LlmSharedState;

use git2::Status;
use log::debug;
use std::path::PathBuf;
use std::sync::Arc;

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

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
    pub file_diff: Option<FileDiff>,
}

#[derive(Debug)]
pub struct GitRepo {
    pub path: PathBuf,
    pub changed_files: Vec<FileDiff>,
    pub staged_files: Vec<FileDiff>,
    pub dirty_directory_files: Vec<FileDiff>,
    pub last_commit_files: Vec<FileDiff>,
    pub last_commit_id: Option<String>,
    pub current_view_mode: ViewMode,
    pub repo_name: String,
    pub branch_name: String,
    pub commit_info: (String, String),
    pub total_stats: (usize, usize, usize),
}

impl Clone for GitRepo {
    fn clone(&self) -> Self {
        Self {
            path: self.path.clone(),
            changed_files: self.changed_files.clone(),
            staged_files: self.staged_files.clone(),
            dirty_directory_files: self.dirty_directory_files.clone(),
            last_commit_files: self.last_commit_files.clone(),
            last_commit_id: self.last_commit_id.clone(),
            current_view_mode: self.current_view_mode,
            repo_name: self.repo_name.clone(),
            branch_name: self.branch_name.clone(),
            commit_info: self.commit_info.clone(),
            total_stats: self.total_stats,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    WorkingTree,
    Staged,
    DirtyDirectory,
    LastCommit,
}

impl GitRepo {
    pub fn get_display_files(&self) -> Vec<FileDiff> {
        match self.current_view_mode {
            ViewMode::WorkingTree => self.changed_files.clone(),
            ViewMode::Staged => self.staged_files.clone(),
            ViewMode::DirtyDirectory => self.dirty_directory_files.clone(),
            ViewMode::LastCommit => self.get_last_commit_files(),
        }
    }

    fn get_last_commit_files(&self) -> Vec<FileDiff> {
        self.last_commit_files.clone()
    }

    pub fn get_file_tree(&self) -> TreeNode {
        let mut root = TreeNode {
            name: ".".to_string(),
            path: self.path.clone(),
            is_dir: true,
            children: Vec::new(),
            file_diff: None,
        };

        for file_diff in &self.get_display_files() {
            self.add_file_to_tree(&mut root, file_diff);
        }

        root
    }

    fn add_file_to_tree(&self, root: &mut TreeNode, file_diff: &FileDiff) {
        let relative_path = if let Ok(rel_path) = file_diff.path.strip_prefix(&self.path) {
            rel_path
        } else {
            &file_diff.path
        };

        let mut current_node = root;

        let components: Vec<_> = relative_path.components().collect();

        for (i, component) in components.iter().enumerate() {
            let component_str = component.as_os_str().to_string_lossy().to_string();

            if i == components.len() - 1 {
                current_node.children.push(TreeNode {
                    name: component_str.clone(),
                    path: file_diff.path.clone(),
                    is_dir: false,
                    children: Vec::new(),
                    file_diff: Some(file_diff.clone()),
                });
            } else {
                let child_index = current_node
                    .children
                    .iter()
                    .position(|child| child.is_dir && child.name == component_str);

                if let Some(index) = child_index {
                    current_node = &mut current_node.children[index];
                } else {
                    let new_child = TreeNode {
                        name: component_str.clone(),
                        path: current_node.path.join(&component_str),
                        is_dir: true,
                        children: Vec::new(),
                        file_diff: None,
                    };
                    current_node.children.push(new_child);
                    let new_len = current_node.children.len();
                    current_node = &mut current_node.children[new_len - 1];
                }
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreloadConfig {
    pub enabled: bool,
    pub count: usize, // Default: 5
}

impl Default for PreloadConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            count: 5,
        }
    }
}

pub struct SummaryPreloader {
    llm_client: Option<crate::llm::LlmClient>,
    config: PreloadConfig,
    llm_state: Arc<crate::shared_state::LlmSharedState>,
}

impl std::fmt::Debug for SummaryPreloader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SummaryPreloader")
            .field("llm_client", &self.llm_client.is_some())
            .field("config", &self.config)
            .field("llm_state", &"Arc<LlmSharedState>")
            .finish()
    }
}

impl SummaryPreloader {
    pub fn new(llm_client: Option<crate::llm::LlmClient>, llm_state: Arc<LlmSharedState>) -> Self {
        Self {
            llm_client,
            config: PreloadConfig::default(),
            llm_state,
        }
    }

    /// Pre-load summaries for a configurable number of commits starting from the beginning
    pub fn preload_summaries(&mut self, commits: &[CommitInfo]) {
        if !self.config.enabled || self.llm_client.is_none() {
            return;
        }

        let count = self.config.count.min(commits.len());
        for commit in commits.iter().take(count) {
            self.preload_single_summary(&commit.sha);
        }
    }

    /// Pre-load summaries around a specific index as user navigates
    pub fn preload_around_index(&mut self, commits: &[CommitInfo], current_index: usize) {
        if !self.config.enabled || self.llm_client.is_none() {
            return;
        }

        let half_count = self.config.count / 2;
        let start_index = current_index.saturating_sub(half_count);
        let end_index = (current_index + half_count + 1).min(commits.len());

        for commit in commits
            .iter()
            .skip(start_index)
            .take(end_index - start_index)
        {
            self.preload_single_summary(&commit.sha);
        }
    }

    /// Pre-load a single commit summary in the background
    fn preload_single_summary(&mut self, commit_sha: &str) {
        // Skip if already loading or no LLM client available
        if self.llm_state.is_summary_loading(commit_sha) || self.llm_client.is_none() {
            return;
        }

        // Check if summary is already cached
        if self.llm_state.get_cached_summary(commit_sha).is_some() {
            debug!(
                "Summary for commit {} already cached, skipping preload",
                commit_sha
            );
            return;
        }

        let sha = commit_sha.to_string();
        let llm_client = self.llm_client.clone();
        let llm_state = Arc::clone(&self.llm_state);

        // Mark as active in shared state
        self.llm_state.start_summary_task(sha.clone());

        // Spawn background task to generate summary
        tokio::spawn(async move {
            Self::generate_summary_with_shared_state(sha, llm_client, llm_state).await;
        });
    }

    /// Generate summary using shared state
    async fn generate_summary_with_shared_state(
        commit_sha: String,
        llm_client: Option<crate::llm::LlmClient>,
        llm_state: Arc<LlmSharedState>,
    ) {
        if let Some(client) = llm_client {
            // Get the full diff using git2 instead of subprocess
            let diff_result = tokio::task::spawn_blocking({
                let commit_sha = commit_sha.clone();
                move || {
                    // Find git repository path
                    let repo_path = std::path::Path::new(".")
                        .ancestors()
                        .find(|p| p.join(".git").exists())
                        .unwrap_or_else(|| std::path::Path::new("."));

                    // Open repository and get diff using git2
                    match git2::Repository::open(repo_path) {
                        Ok(repo) => self::operations::get_full_commit_diff(&repo, &commit_sha),
                        Err(e) => Err(color_eyre::eyre::eyre!("Could not open repository: {}", e)),
                    }
                }
            })
            .await;

            let full_diff = match diff_result {
                Ok(Ok(diff_text)) => diff_text,
                Ok(Err(e)) => {
                    // Git operation failed, log error but continue
                    debug!("Git diff failed for commit {}: {}", commit_sha, e);
                    llm_state.set_error(
                        format!("summary_{}", commit_sha),
                        format!("Git diff failed: {}", e),
                    );
                    llm_state.complete_summary_task(&commit_sha);
                    return;
                }
                Err(e) => {
                    // Task execution failed
                    debug!("Task execution failed for commit {}: {}", commit_sha, e);
                    llm_state.set_error(
                        format!("summary_{}", commit_sha),
                        format!("Task execution failed: {}", e),
                    );
                    llm_state.complete_summary_task(&commit_sha);
                    return;
                }
            };

            // Create a prompt with the full diff content
            let mut prompt =
                "Please provide a brief, 2-sentence summary of what this commit changes:\n\n"
                    .to_string();

            if full_diff.trim().is_empty() {
                prompt.push_str("No diff content available (this might be a merge commit or have parsing issues).\n");
            } else {
                // Limit diff size to prevent overly long prompts (keep first 8000 chars)
                let truncated_diff = if full_diff.len() > 8000 {
                    let truncated = full_diff.chars().take(8000).collect::<String>();
                    format!("{}\n\n[... diff truncated for brevity ...]", truncated)
                } else {
                    full_diff.clone()
                };

                prompt.push_str("Full diff:\n```diff\n");
                prompt.push_str(&truncated_diff);
                prompt.push_str("\n```\n");
            }

            prompt.push_str("\nFocus on the functional impact and purpose of the changes. Keep it concise and technical.");

            // Generate summary with the new API
            match client.get_llm_summary(prompt, full_diff).await {
                Ok(summary_result) => {
                    if !summary_result.has_error {
                        // Cache the summary in shared state
                        let sanitized_summary = summary_result
                            .content
                            .chars()
                            .take(1000)
                            .collect::<String>();
                        llm_state.cache_summary(commit_sha.clone(), sanitized_summary);
                        debug!("Successfully pre-loaded summary for commit {}", commit_sha);
                    } else {
                        // Store error in shared state
                        debug!(
                            "Failed to generate summary for commit {}: {}",
                            commit_sha, summary_result.content
                        );
                        llm_state.set_error(
                            format!("summary_{}", commit_sha),
                            format!("Failed to generate summary: {}", summary_result.content),
                        );
                    }
                }
                Err(e) => {
                    // Store error in shared state
                    debug!(
                        "Failed to generate summary for commit {}: {}",
                        commit_sha, e
                    );
                    llm_state.set_error(
                        format!("summary_{}", commit_sha),
                        format!("Failed to generate summary: {}", e),
                    );
                }
            }

            // Complete the task in shared state regardless of success/failure
            llm_state.complete_summary_task(&commit_sha);
        } else {
            // No LLM client available
            llm_state.set_error(
                format!("summary_{}", commit_sha),
                "No LLM client available".to_string(),
            );
            llm_state.complete_summary_task(&commit_sha);
        }
    }

    /// Update configuration
    pub fn set_config(&mut self, config: PreloadConfig) {
        self.config = config;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_llm_state() -> Arc<LlmSharedState> {
        Arc::new(LlmSharedState::new())
    }

    #[test]
    fn test_preload_config_default() {
        let config = PreloadConfig::default();
        assert!(config.enabled);
        assert_eq!(config.count, 5);
    }

    #[test]
    fn test_summary_preloader_new() {
        let preloader = SummaryPreloader::new(None, create_test_llm_state());
        assert!(preloader.llm_client.is_none());
        assert_eq!(preloader.config.count, 5);
        assert!(preloader.config.enabled);
    }

    #[test]
    fn test_summary_preloader_with_config() {
        let config = PreloadConfig {
            enabled: false,
            count: 10,
        };
        let mut preloader = SummaryPreloader::new(None, create_test_llm_state());
        preloader.set_config(config.clone());
        assert!(!preloader.config.enabled);
        assert_eq!(preloader.config.count, 10);
    }

    #[test]
    fn test_preload_summaries_disabled() {
        let config = PreloadConfig {
            enabled: false,
            count: 5,
        };
        let mut preloader = SummaryPreloader::new(None, create_test_llm_state());
        preloader.set_config(config);

        let commits = vec![CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            files_changed: vec![],
        }];

        // Should not start any tasks when disabled
        preloader.preload_summaries(&commits);
    }

    #[test]
    fn test_preload_summaries_no_llm_client() {
        let mut preloader = SummaryPreloader::new(None, create_test_llm_state());

        let commits = vec![CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            files_changed: vec![],
        }];

        // Should not start any tasks without LLM client
        preloader.preload_summaries(&commits);
    }

    #[test]
    fn test_preload_around_index() {
        let mut preloader = SummaryPreloader::new(None, create_test_llm_state());

        let commits = vec![
            CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "Test commit 1".to_string(),
                files_changed: vec![],
            },
            CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Test commit 2".to_string(),
                files_changed: vec![],
            },
        ];

        // Should not start any tasks without LLM client
        preloader.preload_around_index(&commits, 0);
    }
}
