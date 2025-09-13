use crate::git_worker::GitWorker;
use color_eyre::eyre::Result;
use git2::Status;
use log::debug;
use std::path::PathBuf;
use std::time::{Duration, Instant};
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
}

#[allow(clippy::large_enum_variant)]
pub enum GitWorkerResult {
    Update(GitRepo),
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
}
