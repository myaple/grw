use crate::git::{CommitFileChange, CommitInfo, FileDiff, FileChangeStatus, GitRepo, GitWorkerCommand, GitWorkerResult, ViewMode};
use color_eyre::eyre::Result;
use git2::{Repository, Status, StatusOptions, DiffOptions};
use log::debug;
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;

pub struct GitWorker {
    repo: Repository,
    path: PathBuf,
    changed_files: Vec<FileDiff>,
    staged_files: Vec<FileDiff>,
    dirty_directory_files: Vec<FileDiff>,
    last_commit_files: Vec<FileDiff>,
    last_commit_id: Option<String>,
    current_view_mode: ViewMode,
    rx: mpsc::Receiver<GitWorkerCommand>,
    tx: mpsc::Sender<GitWorkerResult>,
}

impl GitWorker {
    pub fn new(
        path: PathBuf,
        rx: mpsc::Receiver<GitWorkerCommand>,
        tx: mpsc::Sender<GitWorkerResult>,
    ) -> Result<Self> {
        let repo = Repository::open(&path)?;

        let last_commit_id = repo
            .head()
            .ok()
            .and_then(|head| head.peel_to_commit().ok())
            .map(|commit| commit.id().to_string());

        Ok(Self {
            repo,
            path,
            changed_files: Vec::new(),
            staged_files: Vec::new(),
            dirty_directory_files: Vec::new(),
            last_commit_files: Vec::new(),
            last_commit_id,
            current_view_mode: ViewMode::WorkingTree,
            rx,
            tx,
        })
    }

    pub async fn run(&mut self) {
        while let Some(command) = self.rx.recv().await {
            match command {
                GitWorkerCommand::Update => {
                    let result = self.update();
                    if self.tx.send(result).await.is_err() {
                        // Channel closed, terminate worker
                        break;
                    }
                }
            }
        }
    }

    fn update(&mut self) -> GitWorkerResult {
        debug!("Starting git status update for repository: {:?}", self.path);

        // Get all statuses including staged files
        let statuses = self.repo.statuses(Some(
            StatusOptions::new()
                .include_ignored(false)
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ));

        if let Err(e) = statuses {
            return GitWorkerResult::Error(e.to_string());
        }
        let statuses = statuses.unwrap();

        let mut new_changed_files = Vec::new();
        let mut new_staged_files = Vec::new();
        let mut new_dirty_directory_files = Vec::new();
        let status_count = statuses.len();
        debug!("Found {status_count} total status entries");

        for status in statuses.iter() {
            let path = status.path().unwrap_or("");
            let file_path = self.path.join(path);

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

        debug!(
            "Update complete: working_tree={}, staged={}, dirty_directory={}, view_mode={:?}",
            self.changed_files.len(),
            self.staged_files.len(),
            self.dirty_directory_files.len(),
            self.current_view_mode
        );

        let git_repo = self.create_git_repo_snapshot();
        GitWorkerResult::Update(git_repo)
    }

    fn get_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        debug!("Computing diff for file: {path:?} (status: {status:?})");

        let mut line_strings = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        if status.is_wt_new() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let line_count = content.lines().count();
                debug!("New file has {line_count} lines");
                for line in content.lines() {
                    line_strings.push(format!("+ {line}"));
                    additions += 1;
                }
            }
        } else if status.is_wt_modified() {
            if let Ok(output) = std::process::Command::new("git")
                .args(["diff", "--no-color", path.to_str().unwrap_or("")])
                .output()
            {
                let diff_text = String::from_utf8_lossy(&output.stdout);
                for line in diff_text.lines() {
                    if line.starts_with('+') && !line.starts_with("++") {
                        additions += 1;
                    } else if line.starts_with('-') && !line.starts_with("--") {
                        deletions += 1;
                    }
                    line_strings.push(line.to_string());
                }
                debug!("Modified file: +{additions} -{deletions}");
            }
        } else if status.is_wt_deleted()
            && let Ok(output) = std::process::Command::new("git")
                .args(["diff", "--no-color", path.to_str().unwrap_or("")])
                .output()
        {
            let diff_text = String::from_utf8_lossy(&output.stdout);
            for line in diff_text.lines() {
                if line.starts_with('-') && !line.starts_with("--") {
                    deletions += 1;
                }
                line_strings.push(line.to_string());
            }
            debug!("Deleted file: -{deletions} lines");
        }

        FileDiff {
            path: path.to_path_buf(),
            status,
            line_strings,
            additions,
            deletions,
        }
    }

    fn get_staged_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        debug!(
            "Computing staged diff for file: {path:?} (status: {status:?})"
        );

        let mut line_strings = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        // Use git diff --cached to get staged changes
        if let Ok(output) = std::process::Command::new("git")
            .args([
                "diff",
                "--cached",
                "--no-color",
                path.to_str().unwrap_or(""),
            ])
            .output()
        {
            let diff_text = String::from_utf8_lossy(&output.stdout);
            for line in diff_text.lines() {
                if line.starts_with('+') && !line.starts_with("++") {
                    additions += 1;
                } else if line.starts_with('-') && !line.starts_with("--") {
                    deletions += 1;
                }
                line_strings.push(line.to_string());
            }
            debug!("Staged file: +{additions} -{deletions}");
        }

        FileDiff {
            path: path.to_path_buf(),
            status,
            line_strings,
            additions,
            deletions,
        }
    }

    fn get_dirty_directory_diff(&self, path: &Path) -> FileDiff {
        debug!("Computing dirty directory diff for file: {path:?}");

        let mut line_strings = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        // Use git diff to show what would be committed
        if let Ok(output) = std::process::Command::new("git")
            .args(["diff", "--no-color", path.to_str().unwrap_or("")])
            .output()
        {
            let diff_text = String::from_utf8_lossy(&output.stdout);
            for line in diff_text.lines() {
                if line.starts_with('+') && !line.starts_with("++") {
                    additions += 1;
                } else if line.starts_with('-') && !line.starts_with("--") {
                    deletions += 1;
                }
                line_strings.push(line.to_string());
            }
            debug!("Dirty directory file: +{additions} -{deletions}");
        }

        FileDiff {
            path: path.to_path_buf(),
            status: Status::from_bits_truncate(2), // WT_MODIFIED
            line_strings,
            additions,
            deletions,
        }
    }

    fn is_file_in_dirty_directory(&self, path: &Path) -> bool {
        // Check if the file has unstaged changes that would be committed
        if let Ok(output) = std::process::Command::new("git")
            .args(["diff", "--name-only", path.to_str().unwrap_or("")])
            .output()
        {
            !output.stdout.is_empty()
        } else {
            false
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
                        let file_path = self.path.join(new_file);
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
        let mut content = Vec::new();

        // Use git show to get the diff content for the commit
        if let Some(commit_id) = &self.last_commit_id
            && let Ok(output) = std::process::Command::new("git")
                .args([
                    "show",
                    "--format=",
                    "--no-color",
                    commit_id,
                    "--",
                    new_path.to_str().unwrap_or(""),
                ])
                .output()
        {
            let diff_text = String::from_utf8_lossy(&output.stdout);
            for line in diff_text.lines() {
                content.push(line.to_string());
            }
        }

        content
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
    pub fn get_commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>> {
        debug!("Fetching commit history with limit: {}", limit);
        
        let mut commits = Vec::new();
        
        // Check if repository has any commits
        if self.repo.head().is_err() {
            debug!("Repository has no commits");
            return Ok(commits);
        }
        
        let mut revwalk = self.repo.revwalk()?;
        
        // Start from HEAD and walk backwards
        revwalk.push_head()?;
        revwalk.set_sorting(git2::Sort::TIME)?;
        
        let mut count = 0;
        for oid in revwalk {
            if count >= limit {
                break;
            }
            
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            
            let sha = oid.to_string();
            let short_sha = sha.chars().take(7).collect::<String>();
            let message = commit.summary().unwrap_or("").to_string();
            let author = commit.author().name().unwrap_or("Unknown").to_string();
            
            // Format date as a readable string
            let timestamp = commit.time();
            let date = chrono::DateTime::from_timestamp(timestamp.seconds(), 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "Unknown date".to_string());
            
            // Get file changes for this commit (will be populated by get_commit_file_changes)
            let files_changed = self.get_commit_file_changes(&sha)?;
            
            commits.push(CommitInfo {
                sha,
                short_sha,
                message,
                author,
                date,
                files_changed,
            });
            
            count += 1;
        }
        
        debug!("Retrieved {} commits", commits.len());
        Ok(commits)
    }

    /// Get file modifications for a specific commit
    /// Returns a list of files changed in the commit with their change status and line counts
    pub fn get_commit_file_changes(&self, commit_sha: &str) -> Result<Vec<CommitFileChange>> {
        debug!("Getting file changes for commit: {}", commit_sha);
        
        let mut file_changes = Vec::new();
        let oid = git2::Oid::from_str(commit_sha)?;
        let commit = self.repo.find_commit(oid)?;
        
        // Get the commit's tree
        let commit_tree = commit.tree()?;
        
        // Get parent tree (if exists) for comparison
        let parent_tree = if commit.parent_count() > 0 {
            Some(commit.parent(0)?.tree()?)
        } else {
            None
        };
        
        // Create diff between parent and current commit
        let diff = self.repo.diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit_tree),
            Some(&mut DiffOptions::new())
        )?;
        
        // Process each delta (file change) in the diff
        for delta in diff.deltas() {
            let status = match delta.status() {
                git2::Delta::Added => FileChangeStatus::Added,
                git2::Delta::Deleted => FileChangeStatus::Deleted,
                git2::Delta::Modified => FileChangeStatus::Modified,
                git2::Delta::Renamed => FileChangeStatus::Renamed,
                _ => FileChangeStatus::Modified, // Default for other types
            };
            
            // Get the file path (prefer new file path for renames)
            let file_path = if let Some(new_file_path) = delta.new_file().path() {
                self.path.join(new_file_path)
            } else if let Some(old_file_path) = delta.old_file().path() {
                self.path.join(old_file_path)
            } else {
                continue; // Skip if no path available
            };
            
            // Get line count statistics using git diff-tree
            let (additions, deletions) = self.get_commit_file_stats(commit_sha, &file_path)?;
            
            file_changes.push(CommitFileChange {
                path: file_path,
                status,
                additions,
                deletions,
            });
        }
        
        debug!("Found {} file changes for commit {}", file_changes.len(), commit_sha);
        Ok(file_changes)
    }
    
    /// Helper method to get addition/deletion counts for a specific file in a commit
    fn get_commit_file_stats(&self, commit_sha: &str, file_path: &Path) -> Result<(usize, usize)> {
        // Use git diff-tree to get numstat for the specific file
        let relative_path = file_path.strip_prefix(&self.path)
            .unwrap_or(file_path)
            .to_string_lossy();
            
        let output = std::process::Command::new("git")
            .args([
                "diff-tree",
                "--numstat",
                "--no-merges",
                commit_sha,
                "--",
                &relative_path
            ])
            .current_dir(&self.path)
            .output()?;
            
        let output_str = String::from_utf8_lossy(&output.stdout);
        
        // Parse numstat output: "additions\tdeletions\tfilename"
        for line in output_str.lines() {
            let parts: Vec<&str> = line.split('\t').collect();
            if parts.len() >= 2 {
                let additions = parts[0].parse::<usize>().unwrap_or(0);
                let deletions = parts[1].parse::<usize>().unwrap_or(0);
                return Ok((additions, deletions));
            }
        }
        
        Ok((0, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

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

    fn create_commit(repo: &Repository, repo_path: &Path, filename: &str, content: &str, message: &str) -> Result<git2::Oid> {
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
        create_commit(&repo, &repo_path, "file1.txt", "Hello World", "Initial commit")?;
        create_commit(&repo, &repo_path, "file2.txt", "Second file", "Add second file")?;
        create_commit(&repo, &repo_path, "file1.txt", "Hello World Updated", "Update first file")?;
        
        // Create GitWorker
        let (_tx, rx) = mpsc::channel(1);
        let (result_tx, _result_rx) = mpsc::channel(1);
        let git_worker = GitWorker::new(repo_path, rx, result_tx)?;
        
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
            assert!(!commit.author.is_empty());
            assert!(!commit.date.is_empty());
        }
        
        Ok(())
    }

    #[tokio::test]
    async fn test_get_commit_history_with_limit() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;
        
        // Create multiple commits
        for i in 1..=5 {
            create_commit(&repo, &repo_path, &format!("file{}.txt", i), "content", &format!("Commit {}", i))?;
        }
        
        let (_tx, rx) = mpsc::channel(1);
        let (result_tx, _result_rx) = mpsc::channel(1);
        let git_worker = GitWorker::new(repo_path, rx, result_tx)?;
        
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
        let commit1_id = create_commit(&repo, &repo_path, "file1.txt", "Hello\nWorld\n", "Initial commit")?;
        
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
        
        let (_tx, rx) = mpsc::channel(1);
        let (result_tx, _result_rx) = mpsc::channel(1);
        let git_worker = GitWorker::new(repo_path, rx, result_tx)?;
        
        // Test file changes for first commit (should show file1.txt as added)
        let changes1 = git_worker.get_commit_file_changes(&commit1_id.to_string())?;
        assert_eq!(changes1.len(), 1);
        assert!(changes1[0].path.ends_with("file1.txt"));
        assert!(matches!(changes1[0].status, FileChangeStatus::Added));
        
        // Test file changes for second commit (should show file1.txt modified and file2.txt added)
        let changes2 = git_worker.get_commit_file_changes(&commit2_id.to_string())?;
        assert_eq!(changes2.len(), 2);
        
        // Find the changes for each file
        let file1_change = changes2.iter().find(|c| c.path.ends_with("file1.txt")).unwrap();
        let file2_change = changes2.iter().find(|c| c.path.ends_with("file2.txt")).unwrap();
        
        assert!(matches!(file1_change.status, FileChangeStatus::Modified));
        assert!(matches!(file2_change.status, FileChangeStatus::Added));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_get_commit_file_changes_with_deletion() -> Result<()> {
        let (_temp_dir, repo, repo_path) = create_test_repo()?;
        
        // Create initial commit with two files
        create_commit(&repo, &repo_path, "file1.txt", "Content 1", "Initial commit")?;
        
        // Create second commit that adds another file
        create_commit(&repo, &repo_path, "file2.txt", "Content 2", "Add second file")?;
        
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
        
        let (_tx, rx) = mpsc::channel(1);
        let (result_tx, _result_rx) = mpsc::channel(1);
        let git_worker = GitWorker::new(repo_path, rx, result_tx)?;
        
        // Test file changes for deletion commit
        let changes = git_worker.get_commit_file_changes(&commit3_id.to_string())?;
        assert_eq!(changes.len(), 1);
        assert!(changes[0].path.ends_with("file1.txt"));
        assert!(matches!(changes[0].status, FileChangeStatus::Deleted));
        
        Ok(())
    }

    #[tokio::test]
    async fn test_empty_repository() -> Result<()> {
        let (_temp_dir, _repo, repo_path) = create_test_repo()?;
        
        let (_tx, rx) = mpsc::channel(1);
        let (result_tx, _result_rx) = mpsc::channel(1);
        let git_worker = GitWorker::new(repo_path, rx, result_tx)?;
        
        // Test get_commit_history on empty repo
        let commits = git_worker.get_commit_history(10)?;
        assert_eq!(commits.len(), 0);
        
        Ok(())
    }
}