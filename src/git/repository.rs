use std::path::PathBuf;
use super::types::{FileDiff, ViewMode};

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
