use color_eyre::eyre::Result;
use git2::{Repository, Status, StatusOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: Status,
    pub line_strings: Vec<String>,
    pub hunks: Vec<String>,
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
    last_commit_id: Option<String>,
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
            last_commit_id,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        let statuses = self.repo.statuses(Some(
            StatusOptions::new()
                .include_ignored(false)
                .include_untracked(true)
                .recurse_untracked_dirs(true),
        ))?;

        let mut new_changed_files = Vec::new();

        for status in statuses.iter() {
            let path = status.path().unwrap_or("");
            let file_path = self.path.join(path);

            if status.status().is_wt_new()
                || status.status().is_wt_modified()
                || status.status().is_wt_deleted()
            {
                let diff = self.get_file_diff(&file_path, status.status());
                new_changed_files.push(diff);
            }
        }

        self.changed_files = new_changed_files;
        Ok(())
    }

    fn get_file_diff(&self, path: &Path, status: Status) -> FileDiff {
        let mut line_strings = Vec::new();
        let hunks = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;

        if status.is_wt_new() {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    line_strings.push(format!("+ {}", line));
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
            }
        } else if status.is_wt_deleted() {
            if let Ok(output) = std::process::Command::new("git")
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
            }
        }

        FileDiff {
            path: path.to_path_buf(),
            status,
            line_strings,
            hunks,
            additions,
            deletions,
        }
    }

    pub fn get_changed_files(&self) -> &Vec<FileDiff> {
        &self.changed_files
    }

    pub fn get_changed_files_clone(&self) -> Vec<FileDiff> {
        self.changed_files.clone()
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
        let total_files = self.changed_files.len();
        let total_additions: usize = self.changed_files.iter().map(|f| f.additions).sum();
        let total_deletions: usize = self.changed_files.iter().map(|f| f.deletions).sum();
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

        for file_diff in &self.changed_files {
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

    pub fn flatten_tree(&self, tree: &TreeNode) -> Vec<(TreeNode, usize)> {
        let mut result = Vec::new();
        self.flatten_tree_recursive(tree, 0, &mut result);
        result
    }

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

    pub fn get_file_count(&self) -> usize {
        self.changed_files.len()
    }

    pub fn has_changes(&self) -> bool {
        !self.changed_files.is_empty()
    }
}
