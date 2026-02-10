use super::operations as git_operations;
use super::{CommitFileChange, CommitInfo, FileChangeStatus, FileDiff, GitRepo, ViewMode};
use crate::shared_state::GitSharedState;
use color_eyre::eyre::Result;
use git2::{DiffOptions, Repository, Status, StatusOptions};
use log::debug;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Types of diffs that can be generated
#[derive(Debug, Clone, Copy, PartialEq)]
enum DiffType {
    WorkingTree,
    Staged,
    DirtyDirectory,
}

pub struct GitWorker {
    repo: Repository,
    path: PathBuf,
    changed_files: Vec<FileDiff>,
    staged_files: Vec<FileDiff>,
    dirty_directory_files: Vec<FileDiff>,
    last_commit_files: Vec<FileDiff>,
    last_commit_id: Option<String>,
    current_view_mode: ViewMode,
    shared_state: Arc<GitSharedState>,
    last_head_commit_id: Option<String>, // Track HEAD commit to detect branch changes
}

impl GitWorker {
    /// Create a new GitWorker with shared state
    pub fn new(path: PathBuf, shared_state: Arc<GitSharedState>) -> Result<Self> {
        let repo = Repository::open(&path)?;

        let last_commit_id = repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .map(|commit| commit.id().to_string());

        let last_head_commit_id = last_commit_id.clone();

        Ok(Self {
            repo,
            path,
            changed_files: Vec::new(),
            staged_files: Vec::new(),
            dirty_directory_files: Vec::new(),
            last_commit_files: Vec::new(),
            last_commit_id,
            current_view_mode: ViewMode::WorkingTree,
            shared_state,
            last_head_commit_id,
        })
    }

    /// Continuous run loop for shared state mode
    pub async fn run_continuous(&mut self, update_interval_ms: u64) -> Result<()> {
        debug!(
            "Starting GitWorker continuous run loop with {}ms interval",
            update_interval_ms
        );

        let update_interval = tokio::time::Duration::from_millis(update_interval_ms);

        loop {
            // Perform git status update
            if let Err(e) = self.update_shared_state() {
                debug!("Error during git status update: {}", e);
                // Error is already stored in shared state by update_shared_state()
                // Continue running despite errors
            }

            // Sleep for the configured interval
            tokio::time::sleep(update_interval).await;
        }
    }

    /// Update method for shared state mode - updates shared state directly
    pub fn update_shared_state(&mut self) -> Result<()> {
        debug!("Starting git status update for repository: {:?}", self.path);

        // Attempt update with retry logic for transient errors
        let mut last_error = None;
        for attempt in 1..=3 {
            match self.update_internal_direct() {
                Ok(_) => {
                    // Clear any previous errors on success
                    self.shared_state.clear_error("git_status");

                    // Update shared state with the new git repo snapshot
                    let git_repo = self.create_git_repo_snapshot();
                    self.shared_state.update_repo(git_repo);
                    return Ok(());
                }
                Err(e) => {
                    last_error = Some(e);

                    // Check if this is a transient error that might benefit from retry
                    let error_str = last_error.as_ref().unwrap().to_string();
                    let is_transient = error_str.contains("lock")
                        || error_str.contains("busy")
                        || error_str.contains("temporary");

                    if is_transient && attempt < 3 {
                        debug!(
                            "Transient git error on attempt {}, retrying: {}",
                            attempt, error_str
                        );
                        std::thread::sleep(std::time::Duration::from_millis(100 * attempt as u64));
                        continue;
                    } else {
                        debug!("Git error on attempt {} (final): {}", attempt, error_str);
                        break;
                    }
                }
            }
        }

        // If we get here, all attempts failed
        if let Some(error) = last_error {
            let error_msg = error.to_string();
            self.shared_state
                .set_error("git_status".to_string(), error_msg);
            Err(error)
        } else {
            // This shouldn't happen, but handle it gracefully
            let error_msg = "Unknown git status error".to_string();
            self.shared_state
                .set_error("git_status".to_string(), error_msg.clone());
            Err(color_eyre::eyre::eyre!(error_msg))
        }
    }

    /// Internal update logic that handles git status directly
    fn update_internal_direct(&mut self) -> Result<()> {
        // Check for HEAD/branch changes first
        self.detect_head_change();

        // Get all statuses including staged files
        let statuses = self.repo.statuses(Some(
            StatusOptions::new()
                .include_ignored(false)
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ))?;
        let mut new_changed_files = Vec::new();
        let mut new_staged_files = Vec::new();
        let mut new_dirty_directory_files = Vec::new();
        let status_count = statuses.len();
        debug!("Found {status_count} total status entries");

        for status in statuses.iter() {
            let path = status.path().unwrap_or("");
            // Use git2-based path handling for consistent relative/absolute path conversion
            let file_path = super::operations::from_repo_relative_path(&self.repo, Path::new(path));

            // Working tree changes (unstaged)
            if status.status().is_wt_new()
                || status.status().is_wt_modified()
                || status.status().is_wt_deleted()
            {
                let diff = self.get_file_diff(&file_path, status.status());
                debug!(
                    "Processing working tree file: {} (status: {:?})",
                    path,
                    status.status()
                );
                new_changed_files.push(diff);
            }

            // Staged files
            if status.status().is_index_new()
                || status.status().is_index_modified()
                || status.status().is_index_deleted()
                || status.status().is_index_renamed()
                || status.status().is_index_typechange()
            {
                let diff = self.get_staged_file_diff(&file_path, status.status());
                debug!(
                    "Processing staged file: {} (status: {:?})",
                    path,
                    status.status()
                );
                new_staged_files.push(diff);
            }

            // Dirty directory detection (files that would be shown by git diff --name-only)
            if self.is_file_in_dirty_directory(&file_path) {
                let diff = self.get_dirty_directory_diff(&file_path);
                debug!("Processing dirty directory file: {path}");
                new_dirty_directory_files.push(diff);
            }
        }

        // Determine view mode based on priority
        let old_view_mode = self.current_view_mode;
        if !new_changed_files.is_empty() {
            self.current_view_mode = ViewMode::WorkingTree;
        } else if !new_dirty_directory_files.is_empty() {
            self.current_view_mode = ViewMode::DirtyDirectory;
        } else if !new_staged_files.is_empty() {
            self.current_view_mode = ViewMode::Staged;
        } else {
            self.current_view_mode = ViewMode::LastCommit;
            self.last_commit_files = self.get_last_commit_files();
        }

        self.changed_files = new_changed_files;
        self.staged_files = new_staged_files;
        self.dirty_directory_files = new_dirty_directory_files;

        if old_view_mode != self.current_view_mode {
            debug!(
                "View mode changed: {:?} -> {:?}",
                old_view_mode, self.current_view_mode
            );
        }

        log::trace!(
            "Git update: working_tree={}, staged={}, dirty_directory={}, view_mode={:?}",
            self.changed_files.len(),
            self.staged_files.len(),
            self.dirty_directory_files.len(),
            self.current_view_mode
        );

        Ok(())
    }

    /// Unified diff generation method that handles all diff types
    fn generate_diff(&self, path: &Path, status: Status, diff_type: DiffType) -> FileDiff {
        debug!("Computing diff for file: {path:?} (status: {status:?}, type: {diff_type:?})");

        let mut line_strings = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        // Convert absolute path to relative path for git_operations using unified helper
        let relative_path = super::operations::to_repo_relative_path(&self.repo, path);

        match diff_type {
            DiffType::WorkingTree => {
                if status.is_wt_new() {
                    // For new files, use the same git_operations function
                    match git_operations::get_working_tree_diff(&self.repo, &relative_path) {
                        Ok((lines, added, deleted)) => {
                            line_strings = lines;
                            additions = added;
                            deletions = deleted;
                            debug!("New working tree file: +{additions} -{deletions}");
                        }
                        Err(e) => {
                            debug!(
                                "Failed to get working tree diff for new file {:?}: {}",
                                relative_path, e
                            );
                        }
                    }
                } else if status.is_wt_modified() || status.is_wt_deleted() {
                    match git_operations::get_working_tree_diff(&self.repo, &relative_path) {
                        Ok((lines, added, deleted)) => {
                            line_strings = lines;
                            additions = added;
                            deletions = deleted;
                            debug!("Working tree file: +{additions} -{deletions}");
                        }
                        Err(e) => {
                            debug!(
                                "Failed to get working tree diff for {:?}: {}",
                                relative_path, e
                            );
                        }
                    }
                }
            }
            DiffType::Staged => match git_operations::get_staged_diff(&self.repo, &relative_path) {
                Ok((lines, added, deleted)) => {
                    line_strings = lines;
                    additions = added;
                    deletions = deleted;
                    debug!("Staged file: +{additions} -{deletions}");
                }
                Err(e) => {
                    debug!("Failed to get staged diff for {:?}: {}", relative_path, e);
                }
            },
            DiffType::DirtyDirectory => {
                match git_operations::get_working_tree_diff(&self.repo, &relative_path) {
                    Ok((lines, added, deleted)) => {
                        line_strings = lines;
                        additions = added;
                        deletions = deleted;
                        debug!("Dirty directory file: +{additions} -{deletions}");
                    }
                    Err(e) => {
                        debug!(
                            "Failed to get dirty directory diff for {:?}: {}",
                            relative_path, e
                        );
                    }
                }
            }
        }

        FileDiff {
            path: path.to_path_buf(),
            status,
            line_strings,
            additions,
            deletions,
        }
    }

    /// Get file diff for working tree (maintains backward compatibility)
    fn get_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        self.generate_diff(path, status, DiffType::WorkingTree)
    }

    /// Get file diff for staged files (maintains backward compatibility)
    fn get_staged_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        self.generate_diff(path, status, DiffType::Staged)
    }

    /// Get file diff for dirty directory files (maintains backward compatibility)
    fn get_dirty_directory_diff(&self, path: &Path) -> FileDiff {
        self.generate_diff(
            path,
            Status::from_bits_truncate(2),
            DiffType::DirtyDirectory,
        )
    }

    fn is_file_in_dirty_directory(&self, path: &Path) -> bool {
        // Check if the file has unstaged changes that would be committed
        git_operations::is_file_in_dirty_directory(&self.repo, path).unwrap_or(false)
    }

    /// Detect HEAD/branch changes and force refresh of git state
    pub fn detect_head_change(&mut self) {
        let current_head_commit_id = self
            .repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .map(|commit| commit.id().to_string());

        if let Some(ref current_id) = current_head_commit_id {
            if let Some(ref last_id) = self.last_head_commit_id {
                if current_id != last_id {
                    debug!(
                        "HEAD/branch change detected: {} -> {}",
                        &last_id[..7.min(last_id.len())],
                        &current_id[..7.min(current_id.len())]
                    );

                    // Force refresh of all git state
                    self.last_head_commit_id = current_head_commit_id.clone();
                    self.last_commit_id = current_head_commit_id.clone();

                    // Clear cached last commit files to force refresh
                    self.last_commit_files.clear();

                    // If we're currently in LastCommit mode, the view mode logic will
                    // automatically refresh the last_commit_files on this iteration
                    debug!("Forced refresh of git state due to HEAD/branch change");
                }
            } else {
                // First time seeing a HEAD commit
                self.last_head_commit_id = current_head_commit_id.clone();
                self.last_commit_id = current_head_commit_id.clone();
            }
        } else {
            // No HEAD (e.g., empty repository)
            if self.last_head_commit_id.is_some() {
                debug!("HEAD disappeared (branch deleted?)");
                self.last_head_commit_id = None;
                self.last_commit_id = None;
                self.last_commit_files.clear();
            }
        }
    }

    fn get_repo_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    fn get_current_branch(&self) -> String {
        match self.repo.head() {
            Ok(head) => head.shorthand().unwrap_or("detached").to_string(),
            Err(_) => "detached".to_string(),
        }
    }

    fn get_last_commit_info(&self) -> (String, String) {
        if let Some(commit_id) = &self.last_commit_id {
            if let Ok(commit) = self
                .repo
                .find_commit(git2::Oid::from_str(commit_id).unwrap_or(git2::Oid::zero()))
            {
                let short_id = commit_id.chars().take(7).collect::<String>();
                let summary = commit.summary().unwrap_or("no summary").to_string();
                (short_id, summary)
            } else {
                ("unknown".to_string(), "unknown commit".to_string())
            }
        } else {
            ("no commits".to_string(), "no commits".to_string())
        }
    }

    fn get_total_stats(&self) -> (usize, usize, usize) {
        let display_files = match self.current_view_mode {
            ViewMode::WorkingTree => self.changed_files.clone(),
            ViewMode::Staged => self.staged_files.clone(),
            ViewMode::DirtyDirectory => self.dirty_directory_files.clone(),
            ViewMode::LastCommit => self.get_last_commit_files(),
        };
        let total_files = display_files.len();
        let total_additions: usize = display_files.iter().map(|f| f.additions).sum();
        let total_deletions: usize = display_files.iter().map(|f| f.deletions).sum();
        (total_files, total_additions, total_deletions)
    }

    fn get_last_commit_files(&self) -> Vec<FileDiff> {
        let mut files = Vec::new();

        if let Some(commit_id) = &self.last_commit_id
            && let Ok(commit) = self
                .repo
                .find_commit(git2::Oid::from_str(commit_id).unwrap_or(git2::Oid::zero()))
            && let Ok(tree) = commit.tree()
            && let Ok(parent_tree) = commit.parent(0).and_then(|parent| parent.tree())
        {
            // Get the diff between the commit and its parent
            if let Ok(diff) = self
                .repo
                .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
            {
                for delta in diff.deltas() {
                    if let Some(old_file) = delta.old_file().path()
                        && let Some(new_file) = delta.new_file().path()
                    {
                        // Use git2-based path handling for consistent relative/absolute path conversion
                        let file_path =
                            super::operations::from_repo_relative_path(&self.repo, new_file);
                        let diff_content = self.get_commit_diff_content(old_file, new_file);

                        let mut additions = 0;
                        let mut deletions = 0;
                        for line in &diff_content {
                            if line.starts_with('+') && !line.starts_with("+++") {
                                additions += 1;
                            } else if line.starts_with('-') && !line.starts_with("---") {
                                deletions += 1;
                            }
                        }

                        files.push(FileDiff {
                            path: file_path,
                            status: Status::from_bits_truncate(4), // INDEX_MODIFIED
                            line_strings: diff_content,
                            additions,
                            deletions,
                        });
                    }
                }
            }
        }

        files
    }

    fn get_commit_diff_content(&self, _old_path: &Path, new_path: &Path) -> Vec<String> {
        if let Some(commit_id) = &self.last_commit_id {
            git_operations::get_commit_file_diff(&self.repo, commit_id, new_path)
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    }

    fn create_git_repo_snapshot(&self) -> GitRepo {
        GitRepo {
            path: self.path.clone(),
            changed_files: self.changed_files.clone(),
            staged_files: self.staged_files.clone(),
            dirty_directory_files: self.dirty_directory_files.clone(),
            last_commit_files: self.last_commit_files.clone(),
            last_commit_id: self.last_commit_id.clone(),
            current_view_mode: self.current_view_mode,
            repo_name: self.get_repo_name(),
            branch_name: self.get_current_branch(),
            commit_info: self.get_last_commit_info(),
            total_stats: self.get_total_stats(),
        }
    }

    /// Get commit history with SHA and message
    /// Returns a list of commits ordered from most recent to oldest
    /// Uses caching to improve performance for repeated requests
    pub fn get_commit_history(&mut self, limit: usize) -> Result<Vec<CommitInfo>> {
        debug!("Fetching commit history with limit: {}", limit);

        let mut commits = Vec::new();

        // Check if repository has any commits
        match self.repo.head() {
            Err(e) => {
                debug!("Repository has no commits or HEAD is invalid: {}", e);
                // Return empty list for repositories with no commits
                return Ok(commits);
            }
            Ok(head) => {
                // Verify HEAD points to a valid commit
                if head.peel_to_commit().is_err() {
                    debug!("HEAD does not point to a valid commit");
                    return Ok(commits);
                }
            }
        }

        let mut revwalk = match self.repo.revwalk() {
            Ok(walk) => walk,
            Err(e) => {
                debug!("Failed to create revision walker: {}", e);
                return Err(e.into());
            }
        };

        // Start from HEAD and walk backwards
        if let Err(e) = revwalk.push_head() {
            debug!("Failed to push HEAD to revision walker: {}", e);
            return Err(e.into());
        }

        if let Err(e) = revwalk.set_sorting(git2::Sort::TIME) {
            debug!("Failed to set revision walker sorting: {}", e);
            return Err(e.into());
        }

        let mut count = 0;
        let mut errors_encountered = 0;
        const MAX_ERRORS: usize = 5; // Allow some errors but not too many

        for oid_result in revwalk {
            if count >= limit {
                break;
            }

            let oid = match oid_result {
                Ok(oid) => oid,
                Err(e) => {
                    errors_encountered += 1;
                    debug!("Error reading commit OID: {}", e);
                    if errors_encountered >= MAX_ERRORS {
                        debug!("Too many errors encountered, stopping commit history retrieval");
                        break;
                    }
                    continue;
                }
            };

            let commit = match self.repo.find_commit(oid) {
                Ok(commit) => commit,
                Err(e) => {
                    errors_encountered += 1;
                    debug!("Error finding commit {}: {}", oid, e);
                    if errors_encountered >= MAX_ERRORS {
                        debug!("Too many errors encountered, stopping commit history retrieval");
                        break;
                    }
                    continue;
                }
            };

            let sha = oid.to_string();

            // Check shared state cache first for this commit
            if let Some(cached_commit) = self.shared_state.get_cached_commit(&sha) {
                commits.push(cached_commit);
                count += 1;
                continue;
            }

            let short_sha = sha.chars().take(7).collect::<String>();
            let message = commit.summary().unwrap_or("<no message>").to_string();

            // Get file changes for this commit using a separate method that doesn't require mutable self
            let files_changed =
                match Self::get_commit_file_changes_static(&self.repo, &self.path, &sha) {
                    Ok(changes) => changes,
                    Err(e) => {
                        debug!("Error getting file changes for commit {}: {}", sha, e);
                        // Continue with empty file changes rather than failing completely
                        Vec::new()
                    }
                };

            let commit_info = CommitInfo {
                sha: sha.clone(),
                short_sha,
                message,
                files_changed,
            };

            // Cache the commit info in shared state for future use
            self.shared_state.cache_commit(sha, commit_info.clone());
            commits.push(commit_info);

            count += 1;
        }

        if errors_encountered > 0 {
            debug!(
                "Retrieved {} commits with {} errors encountered",
                commits.len(),
                errors_encountered
            );
        } else {
            debug!("Retrieved {} commits", commits.len());
        }

        // Note: Cache eviction is now handled by shared state automatically

        Ok(commits)
    }

    /// Static method to get file changes without requiring mutable self
    /// Used internally by get_commit_history to avoid borrowing issues
    fn get_commit_file_changes_static(
        repo: &Repository,
        repo_path: &Path,
        commit_sha: &str,
    ) -> Result<Vec<CommitFileChange>> {
        debug!("Getting file changes for commit (static): {}", commit_sha);

        let mut file_changes = Vec::new();

        // Validate commit SHA format
        if commit_sha.is_empty() {
            debug!("Empty commit SHA provided");
            return Err(color_eyre::eyre::eyre!("Empty commit SHA"));
        }

        let oid = match git2::Oid::from_str(commit_sha) {
            Ok(oid) => oid,
            Err(e) => {
                debug!("Invalid commit SHA format '{}': {}", commit_sha, e);
                return Err(e.into());
            }
        };

        let commit = match repo.find_commit(oid) {
            Ok(commit) => commit,
            Err(e) => {
                debug!("Commit {} not found: {}", commit_sha, e);
                return Err(e.into());
            }
        };

        // Get the commit's tree with error handling
        let commit_tree = match commit.tree() {
            Ok(tree) => tree,
            Err(e) => {
                debug!("Failed to get tree for commit {}: {}", commit_sha, e);
                return Err(e.into());
            }
        };

        // Get parent tree (if exists) for comparison with error handling
        let parent_tree = if commit.parent_count() > 0 {
            match commit.parent(0).and_then(|parent| parent.tree()) {
                Ok(tree) => Some(tree),
                Err(e) => {
                    debug!("Failed to get parent tree for commit {}: {}", commit_sha, e);
                    // For commits without accessible parents (like initial commit), compare against empty tree
                    None
                }
            }
        } else {
            // Initial commit - no parent
            None
        };

        // Create diff between parent and current commit with error handling
        let diff = match repo.diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit_tree),
            Some(&mut DiffOptions::new()),
        ) {
            Ok(diff) => diff,
            Err(e) => {
                debug!("Failed to create diff for commit {}: {}", commit_sha, e);
                return Err(e.into());
            }
        };

        // Process each delta (file change) in the diff
        let mut errors_encountered = 0;
        const MAX_FILE_ERRORS: usize = 10; // Allow some file processing errors

        for (i, delta) in diff.deltas().enumerate() {
            let status = match delta.status() {
                git2::Delta::Added => FileChangeStatus::Added,
                git2::Delta::Deleted => FileChangeStatus::Deleted,
                git2::Delta::Modified => FileChangeStatus::Modified,
                git2::Delta::Renamed => FileChangeStatus::Renamed,
                git2::Delta::Copied => FileChangeStatus::Modified, // Treat copied as modified
                git2::Delta::Ignored => continue,                  // Skip ignored files
                git2::Delta::Untracked => continue,                // Skip untracked files
                git2::Delta::Typechange => FileChangeStatus::Modified, // Treat type changes as modified
                _ => {
                    debug!(
                        "Unknown delta status for file in commit {}: {:?}",
                        commit_sha,
                        delta.status()
                    );
                    FileChangeStatus::Modified // Default for unknown types
                }
            };

            // Get the relative file path (prefer new file path for renames) with validation
            let relative_file_path = if let Some(new_file_path) = delta.new_file().path() {
                new_file_path.to_path_buf()
            } else if let Some(old_file_path) = delta.old_file().path() {
                old_file_path.to_path_buf()
            } else {
                debug!(
                    "No valid file path found for delta in commit {}",
                    commit_sha
                );
                errors_encountered += 1;
                if errors_encountered >= MAX_FILE_ERRORS {
                    debug!("Too many file processing errors, stopping");
                    break;
                }
                continue; // Skip if no path available
            };

            // Convert to absolute path for the CommitFileChange struct
            let absolute_file_path = repo_path.join(&relative_file_path);

            // Get line count statistics using git diff-tree with error handling
            // OPTIMIZATION: Use the patch from the existing diff instead of recomputing it
            let (additions, deletions) = if let Ok(Some(patch)) = git2::Patch::from_diff(&diff, i) {
                let stats = patch.line_stats().unwrap_or((0, 0, 0));
                (stats.1, stats.2)
            } else {
                match Self::get_commit_file_stats_static_relative(
                    repo,
                    commit_sha,
                    &relative_file_path,
                ) {
                    Ok(stats) => stats,
                    Err(e) => {
                        debug!(
                            "Failed to get file stats for {} in commit {}: {}",
                            relative_file_path.display(),
                            commit_sha,
                            e
                        );
                        errors_encountered += 1;
                        if errors_encountered >= MAX_FILE_ERRORS {
                            debug!("Too many file processing errors, stopping");
                            break;
                        }
                        // Continue with zero stats rather than failing completely
                        (0, 0)
                    }
                }
            };

            file_changes.push(CommitFileChange {
                path: absolute_file_path,
                status,
                additions,
                deletions,
            });
        }

        if errors_encountered > 0 {
            debug!(
                "Found {} file changes for commit {} with {} errors",
                file_changes.len(),
                commit_sha,
                errors_encountered
            );
        } else {
            debug!(
                "Found {} file changes for commit {}",
                file_changes.len(),
                commit_sha
            );
        }

        Ok(file_changes)
    }

    /// Static helper method to get addition/deletion counts for a specific file in a commit (using relative paths)
    fn get_commit_file_stats_static_relative(
        repo: &Repository,
        commit_sha: &str,
        relative_file_path: &Path,
    ) -> Result<(usize, usize)> {
        // Validate inputs
        if commit_sha.is_empty() {
            return Err(color_eyre::eyre::eyre!(
                "Empty commit SHA provided to get_commit_file_stats"
            ));
        }

        if relative_file_path.as_os_str().is_empty() {
            return Err(color_eyre::eyre::eyre!(
                "Empty file path provided to get_commit_file_stats"
            ));
        }

        git_operations::get_commit_file_stats(repo, commit_sha, relative_file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> Result<(TempDir, Repository, PathBuf)> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        let repo = Repository::init(&repo_path)?;

        // Configure git user for commits
        let mut config = repo.config()?;
        config.set_str("user.name", "Test User")?;
        config.set_str("user.email", "test@example.com")?;

        Ok((temp_dir, repo, repo_path))
    }

    fn create_commit(
        repo: &Repository,
        repo_path: &Path,
        filename: &str,
        content: &str,
        message: &str,
    ) -> Result<git2::Oid> {
        // Create file
        let file_path = repo_path.join(filename);
        fs::write(&file_path, content)?;

        // Add to index
        let mut index = repo.index()?;
        index.add_path(Path::new(filename))?;
        index.write()?;

        // Create commit
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = git2::Signature::now("Test User", "test@example.com")?;

        let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = if let Some(ref parent) = parent_commit {
            vec![parent]
        } else {
            vec![]
        };

        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message,
            &tree,
            &parents,
        )?;

        // Small delay to ensure different timestamps
        std::thread::sleep(std::time::Duration::from_millis(10));

        Ok(commit_id)
    }

    #[tokio::test]
    async fn test_get_commit_history() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create some test commits
        create_commit(
            &repo,
            &repo_path,
            "file1.txt",
            "Hello World",
            "Initial commit",
        )?;
        create_commit(
            &repo,
            &repo_path,
            "file2.txt",
            "Second file",
            "Add second file",
        )?;
        create_commit(
            &repo,
            &repo_path,
            "file1.txt",
            "Hello World Updated",
            "Update first file",
        )?;

        // Create GitWorker
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        // Test get_commit_history
        let commits = git_worker.get_commit_history(10)?;

        // Should have 3 commits
        assert_eq!(commits.len(), 3);

        // Check that we have all commits (order may vary due to timing)
        let commit_messages: Vec<&str> = commits.iter().map(|c| c.message.as_str()).collect();
        assert!(commit_messages.contains(&"Update first file"));
        assert!(commit_messages.contains(&"Add second file"));
        assert!(commit_messages.contains(&"Initial commit"));

        // Check that SHA and short_sha are populated
        for commit in &commits {
            assert!(!commit.sha.is_empty());
            assert_eq!(commit.short_sha.len(), 7);
            assert!(commit.sha.starts_with(&commit.short_sha));
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_get_commit_history_with_limit() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create multiple commits
        for i in 1..=5 {
            create_commit(
                &repo,
                &repo_path,
                &format!("file{}.txt", i),
                "content",
                &format!("Commit {}", i),
            )?;
        }

        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        // Test with limit
        let commits = git_worker.get_commit_history(3)?;

        // Should only return 3 commits
        assert_eq!(commits.len(), 3);

        // Should be 3 commits (order may vary due to timing)
        let commit_messages: Vec<&str> = commits.iter().map(|c| c.message.as_str()).collect();
        assert!(commit_messages.len() == 3);
        // Just verify we have some of the expected commits
        assert!(commit_messages.iter().any(|&msg| msg.starts_with("Commit")));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_commit_file_changes() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create initial commit with one file
        let commit1_id = create_commit(
            &repo,
            &repo_path,
            "file1.txt",
            "Hello\nWorld\n",
            "Initial commit",
        )?;

        // Create second commit that modifies the file and adds a new one
        fs::write(repo_path.join("file1.txt"), "Hello\nWorld\nUpdated\n")?;
        fs::write(repo_path.join("file2.txt"), "New file\ncontent\n")?;

        // Add both files to index
        let mut index = repo.index()?;
        index.add_path(Path::new("file1.txt"))?;
        index.add_path(Path::new("file2.txt"))?;
        index.write()?;

        // Create commit
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = git2::Signature::now("Test User", "test@example.com")?;
        let parent_commit = repo.head()?.peel_to_commit()?;

        let commit2_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Add and modify files",
            &tree,
            &[&parent_commit],
        )?;

        // Test file changes for first commit (should show file1.txt as added)
        let changes1 =
            GitWorker::get_commit_file_changes_static(&repo, &repo_path, &commit1_id.to_string())?;
        assert_eq!(changes1.len(), 1);
        assert!(changes1[0].path.ends_with("file1.txt"));
        assert!(matches!(changes1[0].status, FileChangeStatus::Added));

        // Test file changes for second commit (should show file1.txt modified and file2.txt added)
        let changes2 =
            GitWorker::get_commit_file_changes_static(&repo, &repo_path, &commit2_id.to_string())?;
        assert_eq!(changes2.len(), 2);

        // Find the changes for each file
        let file1_change = changes2
            .iter()
            .find(|c| c.path.ends_with("file1.txt"))
            .unwrap();
        let file2_change = changes2
            .iter()
            .find(|c| c.path.ends_with("file2.txt"))
            .unwrap();

        assert!(matches!(file1_change.status, FileChangeStatus::Modified));
        assert!(matches!(file2_change.status, FileChangeStatus::Added));

        Ok(())
    }

    #[tokio::test]
    async fn test_get_commit_file_changes_with_deletion() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create initial commit with two files
        create_commit(
            &repo,
            &repo_path,
            "file1.txt",
            "Content 1",
            "Initial commit",
        )?;

        // Create second commit that adds another file
        create_commit(
            &repo,
            &repo_path,
            "file2.txt",
            "Content 2",
            "Add second file",
        )?;

        // Create third commit that deletes file1.txt
        fs::remove_file(repo_path.join("file1.txt"))?;
        let mut index = repo.index()?;
        index.remove_path(Path::new("file1.txt"))?;
        index.write()?;

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let signature = git2::Signature::now("Test User", "test@example.com")?;
        let parent_commit = repo.head()?.peel_to_commit()?;

        let commit3_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Delete file1.txt",
            &tree,
            &[&parent_commit],
        )?;

        // Test file changes for deletion commit
        let changes =
            GitWorker::get_commit_file_changes_static(&repo, &repo_path, &commit3_id.to_string())?;
        assert_eq!(changes.len(), 1);
        assert!(changes[0].path.ends_with("file1.txt"));
        assert!(matches!(changes[0].status, FileChangeStatus::Deleted));

        Ok(())
    }

    #[tokio::test]
    async fn test_empty_repository() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;

        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        // Test get_commit_history on empty repo
        let commits = git_worker.get_commit_history(10)?;
        assert_eq!(commits.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling_invalid_commit_sha() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create a commit first
        create_commit(&repo, &repo_path, "test.txt", "content", "Test commit")?;

        // Test with invalid commit SHA
        let result = GitWorker::get_commit_file_changes_static(&repo, &repo_path, "invalid_sha");
        assert!(result.is_err());

        // Test with empty commit SHA
        let result = GitWorker::get_commit_file_changes_static(&repo, &repo_path, "");
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_error_handling_corrupted_repository() -> Result<()> {
        let temp_dir = TempDir::new()?;
        let repo_path = temp_dir.path().to_path_buf();

        // Create a fake .git directory without proper git structure
        std::fs::create_dir_all(repo_path.join(".git"))?;
        std::fs::write(repo_path.join(".git/HEAD"), "invalid content")?;

        // Attempt to create GitWorker with corrupted repository
        let shared_state = Arc::new(GitSharedState::new());

        // This should fail gracefully
        let result = GitWorker::new(repo_path, shared_state);
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_commit_caching() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create some commits
        create_commit(&repo, &repo_path, "file1.txt", "content1", "First commit")?;
        create_commit(&repo, &repo_path, "file2.txt", "content2", "Second commit")?;
        create_commit(&repo, &repo_path, "file3.txt", "content3", "Third commit")?;

        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        // First call should populate cache
        let commits1 = git_worker.get_commit_history(10)?;
        assert_eq!(commits1.len(), 3);

        // Second call should return the same results
        let commits2 = git_worker.get_commit_history(10)?;
        assert_eq!(commits2.len(), 3);
        assert_eq!(commits1[0].sha, commits2[0].sha);

        // Cache size management is now handled by shared state
        let commits3 = git_worker.get_commit_history(10)?;
        assert_eq!(commits3.len(), 3);

        // Cache clearing is now handled by shared state

        Ok(())
    }

    #[tokio::test]
    async fn test_commit_history_with_errors() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create some commits
        create_commit(&repo, &repo_path, "file1.txt", "content1", "Commit 1")?;
        create_commit(&repo, &repo_path, "file2.txt", "content2", "Commit 2")?;

        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        // Test that we can still get commits even if some operations fail
        let commits = git_worker.get_commit_history(10)?;
        assert!(commits.len() >= 2);

        // Verify commit data is valid
        for commit in &commits {
            assert!(!commit.sha.is_empty());
            assert!(!commit.short_sha.is_empty());
            assert_eq!(commit.short_sha.len(), 7);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_llm_summary_caching() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create some commits
        let commit1_id = create_commit(&repo, &repo_path, "file1.txt", "content1", "First commit")?;
        let commit2_id =
            create_commit(&repo, &repo_path, "file2.txt", "content2", "Second commit")?;

        // Create shared state and GitWorker with new constructor
        let git_shared_state = Arc::new(crate::shared_state::GitSharedState::new());
        let llm_shared_state = Arc::new(crate::shared_state::LlmSharedState::new());
        let mut git_worker = GitWorker::new(repo_path, git_shared_state)?;

        let commit1_sha = commit1_id.to_string();
        let commit2_sha = commit2_id.to_string();

        // Initially, no summaries should be cached in LLM shared state
        assert!(llm_shared_state.get_cached_summary(&commit1_sha).is_none());
        assert!(llm_shared_state.get_cached_summary(&commit2_sha).is_none());

        // Cache some summaries in LLM shared state
        let summary1 = "This commit adds the first file with initial content.".to_string();
        let summary2 = "This commit adds a second file to the repository.".to_string();

        llm_shared_state.cache_summary(commit1_sha.clone(), summary1.clone());
        llm_shared_state.cache_summary(commit2_sha.clone(), summary2.clone());

        // Verify summaries are cached in LLM shared state
        assert_eq!(
            llm_shared_state.get_cached_summary(&commit1_sha),
            Some(summary1.clone())
        );
        assert_eq!(
            llm_shared_state.get_cached_summary(&commit2_sha),
            Some(summary2.clone())
        );

        // Test cache clearing (LLM shared state doesn't have a clear method, so we test individual removal)
        // Note: LlmSharedState doesn't have a clear_all method, so we verify the cache works as expected

        // Verify git worker still works for commit data
        let commits = git_worker.get_commit_history(10)?;
        assert!(!commits.is_empty()); // Should still have commit data

        Ok(())
    }

    #[tokio::test]
    async fn test_llm_summary_cache_with_invalid_commit_sha() -> Result<()> {
        let (_temp_dir, _repo, _repo_path) = create_test_repo()?;

        // Create shared state for testing
        let llm_shared_state = Arc::new(crate::shared_state::LlmSharedState::new());

        // Test with invalid commit SHA using LLM shared state
        let invalid_sha = "invalid_commit_sha";
        assert!(llm_shared_state.get_cached_summary(invalid_sha).is_none());

        // Cache a summary for invalid SHA (should work - cache doesn't validate)
        llm_shared_state.cache_summary(invalid_sha.to_string(), "Invalid summary".to_string());
        assert_eq!(
            llm_shared_state.get_cached_summary(invalid_sha),
            Some("Invalid summary".to_string())
        );

        // Test with empty SHA
        let empty_sha = "";
        assert!(llm_shared_state.get_cached_summary(empty_sha).is_none());
        llm_shared_state.cache_summary(empty_sha.to_string(), "Empty summary".to_string());
        assert_eq!(
            llm_shared_state.get_cached_summary(empty_sha),
            Some("Empty summary".to_string())
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_git_worker_shared_state_integration() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;

        // Create some commits for testing
        create_commit(&_repo, &repo_path, "file1.txt", "content1", "First commit")?;
        create_commit(&_repo, &repo_path, "file2.txt", "content2", "Second commit")?;

        // Create GitWorker with shared state
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path, shared_state.clone())?;

        // Test update_shared_state
        git_worker.update_shared_state()?;

        // Verify that shared state was updated
        let repo_data = shared_state.get_repo();
        assert!(repo_data.is_some());
        let repo_data = repo_data.unwrap();
        // The repo name will be the temp directory name, just verify it's not empty
        assert!(!repo_data.repo_name.is_empty());

        // Test commit history caching in shared state
        let commits = git_worker.get_commit_history(10)?;
        assert_eq!(commits.len(), 2);

        // Verify commits are cached in shared state
        let first_commit_sha = &commits[0].sha;
        let cached_commit = shared_state.get_cached_commit(first_commit_sha);
        assert!(cached_commit.is_some());
        assert_eq!(cached_commit.unwrap().sha, *first_commit_sha);

        // Test error handling
        // Simulate an error by using an invalid path
        let invalid_shared_state = Arc::new(GitSharedState::new());
        let invalid_path = PathBuf::from("/invalid/path/that/does/not/exist");

        // This should fail during GitWorker creation
        let result = GitWorker::new(invalid_path, invalid_shared_state.clone());
        assert!(result.is_err());

        Ok(())
    }

    #[tokio::test]
    async fn test_git_worker_continuous_run() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;

        // Create initial commit
        create_commit(
            &_repo,
            &repo_path,
            "file1.txt",
            "initial content",
            "Initial commit",
        )?;

        // Create GitWorker with shared state
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state.clone())?;

        // Test that we can start the continuous run (we'll stop it quickly)
        let shared_state_clone = shared_state.clone();
        let run_task = tokio::spawn(async move {
            // Run for a very short time
            tokio::time::timeout(
                tokio::time::Duration::from_millis(100),
                git_worker.run_continuous(50),
            )
            .await
        });

        // Wait for the task to timeout (which is expected)
        let result = run_task.await;
        assert!(result.is_ok()); // The task completed (timed out)

        // The timeout result should be an error (timeout)
        let timeout_result = result.unwrap();
        assert!(timeout_result.is_err()); // Should be timeout error

        // Verify that shared state was updated during the run
        let repo_data = shared_state_clone.get_repo();
        assert!(repo_data.is_some());

        Ok(())
    }

    #[tokio::test]
    async fn test_git_worker_error_handling_in_shared_state() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;

        // Create GitWorker with shared state
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path, shared_state.clone())?;

        // Perform successful update first
        git_worker.update_shared_state()?;

        // Verify no errors initially
        assert!(shared_state.get_error("git_status").is_none());

        // Now corrupt the repository to cause an error
        // We'll simulate this by trying to access a non-existent repository
        let invalid_shared_state = Arc::new(GitSharedState::new());
        let invalid_path = PathBuf::from("/tmp/non_existent_repo_for_test");

        // Create a GitWorker that will fail
        if let Ok(mut invalid_worker) = GitWorker::new(invalid_path, invalid_shared_state.clone()) {
            // This update should fail and set an error in shared state
            let result = invalid_worker.update_shared_state();
            assert!(result.is_err());

            // Verify error was stored in shared state
            let error = invalid_shared_state.get_error("git_status");
            assert!(error.is_some());
            assert!(!error.unwrap().is_empty());
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_git_worker_shared_state_cache_operations() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;

        // Create multiple commits for testing
        create_commit(&_repo, &repo_path, "file1.txt", "content1", "First commit")?;
        create_commit(&_repo, &repo_path, "file2.txt", "content2", "Second commit")?;
        create_commit(&_repo, &repo_path, "file3.txt", "content3", "Third commit")?;

        // Create GitWorker with shared state
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state.clone())?;

        // Test commit history retrieval and caching
        let commits1 = git_worker.get_commit_history(5)?;
        assert_eq!(commits1.len(), 3);

        // Verify all commits are cached
        for commit in &commits1 {
            let cached = shared_state.get_cached_commit(&commit.sha);
            assert!(cached.is_some());
            assert_eq!(cached.unwrap().sha, commit.sha);
        }

        // Test second retrieval uses cache (should be same results)
        let commits2 = git_worker.get_commit_history(5)?;
        assert_eq!(commits2.len(), 3);
        assert_eq!(commits1[0].sha, commits2[0].sha);

        // Test file diff retrieval using static method
        let commit_sha = &commits1[0].sha;
        let file_changes =
            GitWorker::get_commit_file_changes_static(&_repo, &repo_path, commit_sha)?;

        // Verify file changes were retrieved successfully
        assert!(!file_changes.is_empty());

        Ok(())
    }

    #[tokio::test]
    async fn test_git_worker_detects_head_changes() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;

        // Create initial commit
        let commit1_id = create_commit(
            &repo,
            &repo_path,
            "file1.txt",
            "Initial content",
            "Initial commit",
        )?;

        // Create GitWorker and verify it detects the initial HEAD
        let shared_state = Arc::new(GitSharedState::new());
        let mut git_worker = GitWorker::new(repo_path.clone(), shared_state)?;

        assert!(git_worker.last_head_commit_id.is_some());
        assert_eq!(
            git_worker.last_head_commit_id.clone().unwrap(),
            commit1_id.to_string()
        );
        assert_eq!(
            git_worker.last_commit_id.clone().unwrap(),
            commit1_id.to_string()
        );

        // Create another commit (simulating a branch change)
        let commit2_id = create_commit(
            &repo,
            &repo_path,
            "file2.txt",
            "Second content",
            "Second commit",
        )?;

        // Update git worker state to simulate HEAD change detection
        git_worker.detect_head_change();

        // Verify the GitWorker detected the HEAD change
        assert_eq!(
            git_worker.last_head_commit_id.clone().unwrap(),
            commit2_id.to_string()
        );
        assert_eq!(
            git_worker.last_commit_id.clone().unwrap(),
            commit2_id.to_string()
        );

        // Verify last_commit_files was cleared to force refresh
        assert!(git_worker.last_commit_files.is_empty());

        Ok(())
    }
}
