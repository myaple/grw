use color_eyre::eyre::Result;
use git2::{Repository, Status, StatusOptions};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FileDiff {
    pub path: PathBuf,
    pub status: Status,
    pub line_strings: Vec<String>,
    pub hunks: Vec<String>,
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
        
        let last_commit_id = repo.head()
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
                .recurse_untracked_dirs(true)
        ))?;
        
        let mut new_changed_files = Vec::new();
        
        for status in statuses.iter() {
            let path = status.path().unwrap_or("");
            let file_path = self.path.join(path);
            
            if status.status().is_wt_new() || status.status().is_wt_modified() || status.status().is_wt_deleted() {
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
        
        if status.is_wt_new() {
            if let Ok(content) = std::fs::read_to_string(path) {
                for line in content.lines() {
                    line_strings.push(format!("+ {}", line));
                }
            }
        } else if status.is_wt_modified() {
            if let Ok(output) = std::process::Command::new("git")
                .args(["diff", "--no-color", path.to_str().unwrap_or("")])
                .output()
            {
                let diff_text = String::from_utf8_lossy(&output.stdout);
                for line in diff_text.lines() {
                    line_strings.push(line.to_string());
                }
            }
        }
        
        FileDiff {
            path: path.to_path_buf(),
            status,
            line_strings,
            hunks,
        }
    }
    
    pub fn get_changed_files(&self) -> &Vec<FileDiff> {
        &self.changed_files
    }
    
    pub fn get_changed_files_clone(&self) -> Vec<FileDiff> {
        self.changed_files.clone()
    }
    
    pub fn get_file_count(&self) -> usize {
        self.changed_files.len()
    }
    
    pub fn has_changes(&self) -> bool {
        !self.changed_files.is_empty()
    }
}