use color_eyre::eyre::Result;
use git2::{Repository, Status, StatusOptions};
use log::debug;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: Status,
    pub line_strings: Vec<String>,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub children: Vec<TreeNode>,
    pub file_diff: Option<FileDiff>,
}

pub struct GitRepo {
    repo: Repository,
    path: PathBuf,
    changed_files: Vec<FileDiff>,
    staged_files: Vec<FileDiff>,
    dirty_directory_files: Vec<FileDiff>,
    last_commit_id: Option<String>,
    current_view_mode: ViewMode,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewMode {
    WorkingTree,
    Staged,
    DirtyDirectory,
    LastCommit,
}

impl GitRepo {
    pub fn new(path: PathBuf) -> Result<Self> {
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
            last_commit_id,
            current_view_mode: ViewMode::WorkingTree,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        debug!("Starting git status update for repository: {:?}", self.path);

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
        debug!("Found {} total status entries", status_count);

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
                debug!("Processing dirty directory file: {}", path);
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

        Ok(())
    }

    fn get_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        debug!("Computing diff for file: {:?} (status: {:?})", path, status);

        let mut line_strings = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        if status.is_wt_new() {
            if let Ok(content) = std::fs::read_to_string(path) {
                let line_count = content.lines().count();
                debug!("New file has {} lines", line_count);
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
                debug!("Modified file: +{} -{}", additions, deletions);
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
            debug!("Deleted file: -{} lines", deletions);
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
            "Computing staged diff for file: {:?} (status: {:?})",
            path, status
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
            debug!("Staged file: +{} -{}", additions, deletions);
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
        debug!("Computing dirty directory diff for file: {:?}", path);

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
            debug!("Dirty directory file: +{} -{}", additions, deletions);
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

    pub fn get_current_view_mode(&self) -> ViewMode {
        self.current_view_mode
    }

    pub fn get_display_files(&self) -> Vec<FileDiff> {
        match self.current_view_mode {
            ViewMode::WorkingTree => self.changed_files.clone(),
            ViewMode::Staged => self.staged_files.clone(),
            ViewMode::DirtyDirectory => self.dirty_directory_files.clone(),
            ViewMode::LastCommit => self.get_last_commit_files(),
        }
    }

    fn get_last_commit_files(&self) -> Vec<FileDiff> {
        let mut files = Vec::new();

        if let Some(commit_id) = &self.last_commit_id {
            if let Ok(commit) = self
                .repo
                .find_commit(git2::Oid::from_str(commit_id).unwrap_or(git2::Oid::zero()))
            {
                if let Ok(tree) = commit.tree() {
                    if let Ok(parent_tree) = commit.parent(0).and_then(|parent| parent.tree()) {
                        // Get the diff between the commit and its parent
                        if let Ok(diff) =
                            self.repo
                                .diff_tree_to_tree(Some(&parent_tree), Some(&tree), None)
                        {
                            for delta in diff.deltas() {
                                if let Some(old_file) = delta.old_file().path() {
                                    if let Some(new_file) = delta.new_file().path() {
                                        let file_path = self.path.join(new_file);
                                        let diff_content =
                                            self.get_commit_diff_content(old_file, new_file);

                                        files.push(FileDiff {
                                            path: file_path,
                                            status: Status::from_bits_truncate(4), // INDEX_MODIFIED
                                            line_strings: diff_content,
                                            additions: 0, // Would need to parse diff to get accurate counts
                                            deletions: 0,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        files
    }

    fn get_commit_diff_content(&self, _old_path: &Path, new_path: &Path) -> Vec<String> {
        let mut content = Vec::new();

        // Use git show to get the diff content for the commit
        if let Some(commit_id) = &self.last_commit_id {
            if let Ok(output) = std::process::Command::new("git")
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
        }

        content
    }

    pub fn get_repo_name(&self) -> String {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("unknown")
            .to_string()
    }

    pub fn get_current_branch(&self) -> String {
        match self.repo.head() {
            Ok(head) => head.shorthand().unwrap_or("detached").to_string(),
            Err(_) => "detached".to_string(),
        }
    }

    pub fn get_last_commit_info(&self) -> (String, String) {
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

    pub fn get_total_stats(&self) -> (usize, usize, usize) {
        let display_files = self.get_display_files();
        let total_files = display_files.len();
        let total_additions: usize = display_files.iter().map(|f| f.additions).sum();
        let total_deletions: usize = display_files.iter().map(|f| f.deletions).sum();
        (total_files, total_additions, total_deletions)
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

    #[allow(dead_code)]
    pub fn flatten_tree(&self, tree: &TreeNode) -> Vec<(TreeNode, usize)> {
        let mut result = Vec::new();
        self.flatten_tree_recursive(tree, 0, &mut result);
        result
    }

    #[allow(clippy::only_used_in_recursion)]
    fn flatten_tree_recursive(
        &self,
        node: &TreeNode,
        depth: usize,
        result: &mut Vec<(TreeNode, usize)>,
    ) {
        if node.file_diff.is_some() || !node.children.is_empty() {
            result.push((node.clone(), depth));
        }

        for child in &node.children {
            self.flatten_tree_recursive(child, depth + 1, result);
        }
    }
}
