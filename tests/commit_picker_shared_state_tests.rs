use std::sync::Arc;
use std::path::PathBuf;
use tempfile::TempDir;
use git2::Repository;

use grw::shared_state::{SharedStateManager, GitSharedState, LlmSharedState};
use grw::git::{CommitInfo, GitRepo, ViewMode};

#[tokio::test]
async fn test_commit_picker_shared_state_integration() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize git repository
    let repo = Repository::init(&repo_path).unwrap();
    
    // Create initial commit
    let sig = git2::Signature::now("Test User", "test@example.com").unwrap();
    let tree_id = {
        let mut index = repo.index().unwrap();
        index.write_tree().unwrap()
    };
    let tree = repo.find_tree(tree_id).unwrap();
    let _commit = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Initial commit",
        &tree,
        &[],
    ).unwrap();
    
    // Initialize shared state
    let shared_state = SharedStateManager::new();
    shared_state.initialize().unwrap();
    
    // Create test commit info
    let test_commit = CommitInfo {
        sha: "abc123".to_string(),
        short_sha: "abc123".to_string(),
        message: "Test commit".to_string(),
        author: "Test Author".to_string(),
        date: "2023-01-01".to_string(),
        files_changed: vec![],
    };
    
    // Cache commit in shared state
    shared_state.git_state().cache_commit(test_commit.sha.clone(), test_commit.clone());
    
    // Verify commit is cached
    let cached_commit = shared_state.git_state().get_cached_commit(&test_commit.sha);
    assert!(cached_commit.is_some());
    assert_eq!(cached_commit.unwrap().message, "Test commit");
}

#[tokio::test]
async fn test_commit_summary_shared_state_caching() {
    // Initialize shared state
    let shared_state = SharedStateManager::new();
    shared_state.initialize().unwrap();
    
    let commit_sha = "abc123".to_string();
    let summary = "This is a test summary".to_string();
    
    // Cache summary in shared state
    shared_state.llm_state().cache_summary(commit_sha.clone(), summary.clone());
    
    // Verify summary is cached
    let cached_summary = shared_state.llm_state().get_cached_summary(&commit_sha);
    assert!(cached_summary.is_some());
    assert_eq!(cached_summary.unwrap(), summary);
}

#[tokio::test]
async fn test_commit_picker_error_handling_shared_state() {
    // Initialize shared state
    let shared_state = SharedStateManager::new();
    shared_state.initialize().unwrap();
    
    // Set an error in git shared state
    let error_message = "Test git error".to_string();
    shared_state.git_state().set_error("commit_history".to_string(), error_message.clone());
    
    // Verify error is stored
    let stored_error = shared_state.git_state().get_error("commit_history");
    assert!(stored_error.is_some());
    assert_eq!(stored_error.unwrap(), error_message);
    
    // Clear error
    let cleared = shared_state.git_state().clear_error("commit_history");
    assert!(cleared);
    
    // Verify error is cleared
    let cleared_error = shared_state.git_state().get_error("commit_history");
    assert!(cleared_error.is_none());
}

#[tokio::test]
async fn test_commit_picker_concurrent_access() {
    // Initialize shared state
    let shared_state = Arc::new(SharedStateManager::new());
    shared_state.initialize().unwrap();
    
    let commit_sha = "abc123".to_string();
    let summary = "Concurrent test summary".to_string();
    
    // Test concurrent access to LLM shared state
    let shared_state_clone = shared_state.clone();
    let commit_sha_clone = commit_sha.clone();
    let summary_clone = summary.clone();
    
    let handle1 = tokio::spawn(async move {
        shared_state_clone.llm_state().cache_summary(commit_sha_clone, summary_clone);
    });
    
    let shared_state_clone2 = shared_state.clone();
    let commit_sha_clone2 = commit_sha.clone();
    
    let handle2 = tokio::spawn(async move {
        // Try to read while writing
        tokio::time::sleep(tokio::time::Duration::from_millis(1)).await;
        shared_state_clone2.llm_state().get_cached_summary(&commit_sha_clone2)
    });
    
    // Wait for both operations to complete
    handle1.await.unwrap();
    let result = handle2.await.unwrap();
    
    // Verify the summary was cached successfully
    assert!(result.is_some());
    assert_eq!(result.unwrap(), summary);
}

#[tokio::test]
async fn test_git_repo_shared_state_update() {
    // Create a temporary git repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_path_buf();
    
    // Initialize git repository
    let _repo = Repository::init(&repo_path).unwrap();
    
    // Initialize shared state
    let shared_state = SharedStateManager::new();
    shared_state.initialize().unwrap();
    
    // Create test git repo
    let test_repo = GitRepo {
        path: repo_path.clone(),
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
    
    // Update repo in shared state
    shared_state.git_state().update_repo(test_repo.clone());
    
    // Verify repo is stored
    let stored_repo = shared_state.git_state().get_repo();
    assert!(stored_repo.is_some());
    let stored_repo = stored_repo.unwrap();
    assert_eq!(stored_repo.repo_name, "test-repo");
    assert_eq!(stored_repo.branch_name, "main");
    assert_eq!(stored_repo.path, repo_path);
}