use crate::git_worker::GitWorker;
use color_eyre::eyre::Result;
use git2::Status;
use log::debug;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::collections::HashSet;
use tokio::sync::mpsc;

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
    pub author: String,
    pub date: String,
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

pub enum GitWorkerCommand {
    Update,
    GetCommitHistory(usize), // limit
    CacheSummary(String, String), // commit_sha, summary
    GetCachedSummary(String), // commit_sha
}

#[allow(clippy::large_enum_variant)]
pub enum GitWorkerResult {
    Update(GitRepo),
    CommitHistory(Vec<CommitInfo>),
    CachedSummary(Option<String>),
    SummaryCached,
    Error(String),
}

pub struct AsyncGitRepo {
    tx: mpsc::Sender<GitWorkerCommand>,
    rx: mpsc::Receiver<GitWorkerResult>,
    last_update: Instant,
    update_interval: Duration,
    pub repo: Option<GitRepo>,
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

    pub fn get_diff_string(&self) -> String {
        self.get_display_files()
            .iter()
            .map(|f| {
                let mut diff_content = format!(
                    "diff --git a/{} b/{}\n",
                    f.path.to_string_lossy(),
                    f.path.to_string_lossy()
                );
                diff_content.push_str(&f.line_strings.join("\n"));
                diff_content
            })
            .collect::<Vec<String>>()
            .join("\n")
    }
}

impl AsyncGitRepo {
    pub fn new(path: PathBuf, update_interval_ms: u64) -> Result<Self> {
        let (worker_tx, worker_rx) = mpsc::channel(1);
        let (result_tx, result_rx) = mpsc::channel(1);

        let mut worker = GitWorker::new(path.clone(), worker_rx, result_tx)?;
        tokio::spawn(async move {
            worker.run().await;
        });

        Ok(Self {
            tx: worker_tx,
            rx: result_rx,
            last_update: Instant::now(),
            update_interval: Duration::from_millis(update_interval_ms),
            repo: None,
        })
    }

    pub fn update(&mut self) {
        if self.last_update.elapsed() >= self.update_interval {
            debug!("Requesting git update");
            if self.tx.try_send(GitWorkerCommand::Update).is_ok() {
                self.last_update = Instant::now();
            }
        }
    }

    pub fn try_get_result(&mut self) -> Option<GitWorkerResult> {
        self.rx.try_recv().ok()
    }

    pub fn get_git_worker_tx(&self) -> mpsc::Sender<GitWorkerCommand> {
        self.tx.clone()
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
    active_tasks: HashSet<String>,
    git_worker_tx: Option<mpsc::Sender<GitWorkerCommand>>,
}

impl std::fmt::Debug for SummaryPreloader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SummaryPreloader")
            .field("llm_client", &self.llm_client.is_some())
            .field("config", &self.config)
            .field("active_tasks", &self.active_tasks)
            .field("git_worker_tx", &self.git_worker_tx.is_some())
            .finish()
    }
}

impl SummaryPreloader {
    pub fn new(llm_client: Option<crate::llm::LlmClient>) -> Self {
        Self {
            llm_client,
            config: PreloadConfig::default(),
            active_tasks: HashSet::new(),
            git_worker_tx: None,
        }
    }

    pub fn new_with_config(llm_client: Option<crate::llm::LlmClient>, config: PreloadConfig) -> Self {
        Self {
            llm_client,
            config,
            active_tasks: HashSet::new(),
            git_worker_tx: None,
        }
    }

    pub fn set_git_worker_tx(&mut self, tx: mpsc::Sender<GitWorkerCommand>) {
        self.git_worker_tx = Some(tx);
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

        for commit in commits.iter().skip(start_index).take(end_index - start_index) {
            self.preload_single_summary(&commit.sha);
        }
    }

    /// Check if a summary is currently being loaded
    pub fn is_loading(&self, commit_sha: &str) -> bool {
        self.active_tasks.contains(commit_sha)
    }

    /// Pre-load a single commit summary in the background
    fn preload_single_summary(&mut self, commit_sha: &str) {
        // Skip if already loading or no LLM client available
        if self.active_tasks.contains(commit_sha) || self.llm_client.is_none() {
            return;
        }

        // Check if summary is already cached
        if let Some(git_worker_tx) = &self.git_worker_tx {
            let sha = commit_sha.to_string();
            let tx = git_worker_tx.clone();
            let llm_client = self.llm_client.clone();
            
            // Mark as active
            self.active_tasks.insert(commit_sha.to_string());

            // Spawn background task to check cache and generate if needed
            let mut active_tasks_clone = self.active_tasks.clone();
            tokio::spawn(async move {
                // First check if it's already cached
                if let Ok(()) = tx.send(GitWorkerCommand::GetCachedSummary(sha.clone())).await {
                    // The result will be handled by the main app loop
                    // If not cached, we'll generate it
                    Self::generate_summary_if_needed(sha, llm_client, tx, &mut active_tasks_clone).await;
                }
            });
        }
    }

    /// Generate summary if not already cached
    async fn generate_summary_if_needed(
        commit_sha: String,
        llm_client: Option<crate::llm::LlmClient>,
        git_worker_tx: mpsc::Sender<GitWorkerCommand>,
        active_tasks: &mut HashSet<String>,
    ) {
        if let Some(client) = llm_client {
            // Get the full diff using git show command
            let diff_result = tokio::task::spawn_blocking({
                let commit_sha = commit_sha.clone();
                move || {
                    std::process::Command::new("git")
                        .args([
                            "show",
                            "--format=", // Don't show commit message, just the diff
                            "--no-color",
                            &commit_sha,
                        ])
                        .output()
                }
            })
            .await;

            let full_diff = match diff_result {
                Ok(Ok(output)) if output.status.success() => {
                    String::from_utf8_lossy(&output.stdout).to_string()
                }
                Ok(Ok(output)) => {
                    // Git command failed, log error but continue
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    debug!("Git show failed for commit {}: {}", commit_sha, stderr);
                    active_tasks.remove(&commit_sha);
                    return;
                }
                Ok(Err(e)) => {
                    // Failed to execute git command
                    debug!("Failed to execute git show for commit {}: {}", commit_sha, e);
                    active_tasks.remove(&commit_sha);
                    return;
                }
                Err(e) => {
                    // Task execution failed
                    debug!("Task execution failed for commit {}: {}", commit_sha, e);
                    active_tasks.remove(&commit_sha);
                    return;
                }
            };

            // Create a prompt with the full diff content
            let mut prompt = "Please provide a brief, 2-sentence summary of what this commit changes:\n\n".to_string();
            
            if full_diff.trim().is_empty() {
                prompt.push_str("No diff content available (this might be a merge commit or have parsing issues).\n");
            } else {
                // Limit diff size to prevent overly long prompts (keep first 8000 chars)
                let truncated_diff = if full_diff.len() > 8000 {
                    let truncated = full_diff.chars().take(8000).collect::<String>();
                    format!("{}\n\n[... diff truncated for brevity ...]", truncated)
                } else {
                    full_diff
                };

                prompt.push_str("Full diff:\n```diff\n");
                prompt.push_str(&truncated_diff);
                prompt.push_str("\n```\n");
            }

            prompt.push_str("\nFocus on the functional impact and purpose of the changes. Keep it concise and technical.");

            let history = vec![openai_api_rs::v1::chat_completion::ChatCompletionMessage {
                role: openai_api_rs::v1::chat_completion::MessageRole::user,
                content: openai_api_rs::v1::chat_completion::Content::Text(prompt),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }];

            // Generate summary
            match client.get_llm_summary(history).await {
                Ok(summary) => {
                    // Cache the summary
                    let sanitized_summary = summary.chars().take(1000).collect::<String>();
                    let _ = git_worker_tx.send(GitWorkerCommand::CacheSummary(commit_sha.clone(), sanitized_summary)).await;
                    debug!("Successfully pre-loaded summary for commit {}", commit_sha);
                }
                Err(e) => {
                    // Log error but don't block UI
                    debug!("Failed to generate summary for commit {}: {}", commit_sha, e);
                }
            }

            // Remove from active tasks regardless of success/failure
            active_tasks.remove(&commit_sha);
        }
    }

    /// Update configuration
    pub fn set_config(&mut self, config: PreloadConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn get_config(&self) -> &PreloadConfig {
        &self.config
    }

    /// Clear all active tasks (useful for cleanup)
    pub fn clear_active_tasks(&mut self) {
        self.active_tasks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_preload_config_default() {
        let config = PreloadConfig::default();
        assert!(config.enabled);
        assert_eq!(config.count, 5);
    }

    #[test]
    fn test_summary_preloader_new() {
        let preloader = SummaryPreloader::new(None);
        assert!(preloader.llm_client.is_none());
        assert_eq!(preloader.config.count, 5);
        assert!(preloader.config.enabled);
        assert!(preloader.active_tasks.is_empty());
    }

    #[test]
    fn test_summary_preloader_with_config() {
        let config = PreloadConfig {
            enabled: false,
            count: 10,
        };
        let preloader = SummaryPreloader::new_with_config(None, config.clone());
        assert_eq!(preloader.config.enabled, false);
        assert_eq!(preloader.config.count, 10);
    }

    #[test]
    fn test_preload_summaries_disabled() {
        let config = PreloadConfig {
            enabled: false,
            count: 5,
        };
        let mut preloader = SummaryPreloader::new_with_config(None, config);
        
        let commits = vec![CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        }];

        // Should not start any tasks when disabled
        preloader.preload_summaries(&commits);
        assert!(preloader.active_tasks.is_empty());
    }

    #[test]
    fn test_preload_summaries_no_llm_client() {
        let mut preloader = SummaryPreloader::new(None);
        
        let commits = vec![CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        }];

        // Should not start any tasks without LLM client
        preloader.preload_summaries(&commits);
        assert!(preloader.active_tasks.is_empty());
    }

    #[test]
    fn test_preload_around_index() {
        let mut preloader = SummaryPreloader::new(None);
        
        let commits = vec![
            CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "Test commit 1".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-01".to_string(),
                files_changed: vec![],
            },
            CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Test commit 2".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-02".to_string(),
                files_changed: vec![],
            },
        ];

        // Should not start any tasks without LLM client
        preloader.preload_around_index(&commits, 0);
        assert!(preloader.active_tasks.is_empty());
    }

    #[test]
    fn test_is_loading() {
        let mut preloader = SummaryPreloader::new(None);
        
        // Initially nothing is loading
        assert!(!preloader.is_loading("abc123"));
        
        // Manually add to active tasks for testing
        preloader.active_tasks.insert("abc123".to_string());
        assert!(preloader.is_loading("abc123"));
        assert!(!preloader.is_loading("def456"));
    }

    #[test]
    fn test_clear_active_tasks() {
        let mut preloader = SummaryPreloader::new(None);
        
        // Add some active tasks
        preloader.active_tasks.insert("abc123".to_string());
        preloader.active_tasks.insert("def456".to_string());
        assert_eq!(preloader.active_tasks.len(), 2);
        
        // Clear all tasks
        preloader.clear_active_tasks();
        assert!(preloader.active_tasks.is_empty());
    }
}
