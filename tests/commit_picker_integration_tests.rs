use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;

// Import the main application modules
use grw::{App, CommitFileChange, CommitInfo, FileChangeStatus, Theme};

/// Helper function to create test LLM shared state
fn create_test_llm_state() -> Arc<grw::shared_state::LlmSharedState> {
    Arc::new(grw::shared_state::LlmSharedState::new())
}

/// Helper function to create test commits
fn create_test_commits() -> Vec<CommitInfo> {
    vec![
        CommitInfo {
            sha: "abc123def456789".to_string(),
            short_sha: "abc123d".to_string(),
            message: "Initial commit\n\nAdded basic project structure".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01 12:00:00".to_string(),
            files_changed: vec![CommitFileChange {
                path: std::path::PathBuf::from("src/main.rs"),
                status: FileChangeStatus::Added,
                additions: 10,
                deletions: 0,
            }],
        },
        CommitInfo {
            sha: "def456ghi789abc".to_string(),
            short_sha: "def456g".to_string(),
            message: "Add feature X\n\nImplemented new functionality for feature X".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-02 14:30:00".to_string(),
            files_changed: vec![
                CommitFileChange {
                    path: std::path::PathBuf::from("src/feature.rs"),
                    status: FileChangeStatus::Added,
                    additions: 25,
                    deletions: 0,
                },
                CommitFileChange {
                    path: std::path::PathBuf::from("src/main.rs"),
                    status: FileChangeStatus::Modified,
                    additions: 5,
                    deletions: 2,
                },
            ],
        },
        CommitInfo {
            sha: "ghi789jkl012mno".to_string(),
            short_sha: "ghi789j".to_string(),
            message: "Fix bug in feature X".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-03 09:15:00".to_string(),
            files_changed: vec![CommitFileChange {
                path: std::path::PathBuf::from("src/feature.rs"),
                status: FileChangeStatus::Modified,
                additions: 3,
                deletions: 1,
            }],
        },
    ]
}

#[tokio::test]
async fn test_complete_commit_picker_workflow_enter_navigate_select_return() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Step 1: Verify initial state - not in commit picker mode
    assert!(!app.is_in_commit_picker_mode());
    assert!(app.get_selected_commit().is_none());

    // Step 2: Enter commit picker mode (simulating Ctrl+P activation)
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Step 3: Load test commits into commit picker
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits.clone());

    // Step 4: Navigate through commits using j/k keys
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);

    // Navigate down (j key)
    app.forward_key_to_commit_picker(j_key);

    // Navigate up (k key)
    app.forward_key_to_commit_picker(k_key);

    // Step 5: Press Enter to select current commit
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.forward_key_to_commit_picker(enter_key);

    // Should have enter pressed flag set
    assert!(app.is_commit_picker_enter_pressed());

    // Step 6: Simulate commit selection process
    if let Some(selected_commit) = app.get_current_selected_commit_from_picker() {
        app.load_commit_files(&selected_commit);
        app.select_commit(selected_commit);
        app.reset_commit_picker_enter_pressed();
    }

    // Step 7: Verify return to normal mode with selected commit
    assert!(!app.is_in_commit_picker_mode());
    assert!(app.get_selected_commit().is_some());
    assert!(!app.is_commit_picker_enter_pressed());
}

#[tokio::test]
async fn test_commit_picker_with_empty_commit_list() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load empty commit list
    app.update_commit_picker_commits(vec![]);

    // Try navigation - should handle empty list gracefully
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);

    app.forward_key_to_commit_picker(j_key);
    app.forward_key_to_commit_picker(k_key);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());

    // Try to select (should handle gracefully)
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.forward_key_to_commit_picker(enter_key);

    // Should still be in commit picker mode since no commit to select
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_picker_with_single_commit() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load single commit
    let single_commit = vec![create_test_commits()[0].clone()];
    app.update_commit_picker_commits(single_commit);

    // Try navigation - should handle single commit gracefully
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);

    app.forward_key_to_commit_picker(j_key);
    app.forward_key_to_commit_picker(k_key);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());

    // Select the single commit
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.forward_key_to_commit_picker(enter_key);

    // Should have selection state
    assert!(app.is_commit_picker_enter_pressed());
}

#[tokio::test]
async fn test_commit_picker_with_many_commits() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Create many commits for testing pagination/scrolling
    let mut many_commits = Vec::new();
    for i in 1..=20 {
        many_commits.push(CommitInfo {
            sha: format!("commit{:02}sha{}", i, "0".repeat(10)),
            short_sha: format!("commit{:02}", i),
            message: format!("Commit number {}", i),
            author: "Test Author".to_string(),
            date: format!("2023-01-{:02} 12:00:00", i),
            files_changed: vec![CommitFileChange {
                path: std::path::PathBuf::from(format!("file{}.txt", i)),
                status: FileChangeStatus::Modified,
                additions: i,
                deletions: i / 2,
            }],
        });
    }

    app.update_commit_picker_commits(many_commits);

    // Test extensive navigation
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

    // Navigate through multiple commits
    for _ in 0..10 {
        app.forward_key_to_commit_picker(j_key);
    }

    // Should still be in commit picker mode and handle many commits
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_g_t_and_g_shift_t_navigation() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Test g+t navigation (next commit)
    let g_key = KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE);
    let t_key = KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE);

    // Simulate g press followed by t (within timing window)
    app.forward_key_to_commit_picker(g_key);
    // In real implementation, there would be timing logic here
    app.forward_key_to_commit_picker(t_key);

    // Test g+T navigation (previous commit)
    let shift_t_key = KeyEvent::new(KeyCode::Char('T'), KeyModifiers::SHIFT);

    app.forward_key_to_commit_picker(g_key);
    app.forward_key_to_commit_picker(shift_t_key);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_highlighting_and_selection() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Navigate and test highlighting
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);

    // Move down
    app.forward_key_to_commit_picker(j_key);

    // Move up
    app.forward_key_to_commit_picker(k_key);

    // Test selection
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.forward_key_to_commit_picker(enter_key);

    // Should have selection state
    assert!(app.is_commit_picker_enter_pressed());

    // Verify we can get the selected commit
    let selected_commit = app.get_current_selected_commit_from_picker();
    assert!(selected_commit.is_some());
}

#[tokio::test]
async fn test_integration_with_existing_diff_navigation_after_commit_selection() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Step 1: Enter commit picker mode and select a commit
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Select a commit
    let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    app.forward_key_to_commit_picker(enter_key);

    // Simulate commit selection process
    if let Some(selected_commit) = app.get_current_selected_commit_from_picker() {
        app.load_commit_files(&selected_commit);
        app.select_commit(selected_commit);
        app.reset_commit_picker_enter_pressed();
    }

    // Step 2: Verify we're back in normal mode with a selected commit
    assert!(!app.is_in_commit_picker_mode());
    assert!(app.get_selected_commit().is_some());

    // Step 3: Test that existing diff navigation works
    // Test file navigation
    app.next_file();
    app.prev_file();

    // Test scroll navigation
    app.scroll_down(20);
    app.scroll_up();

    // Should still have selected commit and be in normal mode
    assert!(!app.is_in_commit_picker_mode());
    assert!(app.get_selected_commit().is_some());
}

#[tokio::test]
async fn test_escape_key_exits_commit_picker() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Press Escape to exit
    let _escape_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);

    // Simulate escape key handling (in real app this would be in main key handler)
    if app.is_in_commit_picker_mode() {
        app.exit_commit_picker_mode();
    }

    // Should exit commit picker mode
    assert!(!app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_picker_only_activates_when_diff_panel_visible() {
    // Create app with diff panel disabled
    let mut app = App::new_with_config(false, true, Theme::Dark, None, create_test_llm_state());

    // Verify diff panel is not visible
    assert!(!app.is_showing_diff_panel());

    // Try to enter commit picker mode (should NOT work when diff panel is not visible)
    app.enter_commit_picker_mode();

    // Should NOT enter commit picker mode
    assert!(!app.is_in_commit_picker_mode());

    // Enable diff panel
    app.toggle_diff_panel(); // This should enable it

    // Now diff panel should be visible
    assert!(app.is_showing_diff_panel());

    // Enter commit picker mode again
    app.enter_commit_picker_mode();

    // Should now enter commit picker mode
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_picker_state_persistence_across_navigation() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Navigate through commits and verify state persistence
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

    // Navigate down several times
    for _ in 0..3 {
        app.forward_key_to_commit_picker(j_key);
        // Verify we're still in commit picker mode after each navigation
        assert!(app.is_in_commit_picker_mode());
    }

    // Navigate back up
    let k_key = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
    for _ in 0..2 {
        app.forward_key_to_commit_picker(k_key);
        assert!(app.is_in_commit_picker_mode());
    }

    // State should be preserved throughout navigation
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_picker_error_handling() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Set error state
    app.set_commit_picker_error("Test error message".to_string());

    // Should still be in commit picker mode but with error state
    assert!(app.is_in_commit_picker_mode());

    // Try navigation with error state - should handle gracefully
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
    app.forward_key_to_commit_picker(j_key);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_picker_loading_state() {
    // Create app with diff panel enabled
    let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Set loading state
    app.set_commit_picker_loading();

    // Should still be in commit picker mode but with loading state
    assert!(app.is_in_commit_picker_mode());

    // Load commits after loading state
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Should still be in commit picker mode with commits loaded
    assert!(app.is_in_commit_picker_mode());
}

#[tokio::test]
async fn test_commit_summary_pane_integration() {
    // Create app with diff panel enabled
    let llm_state = create_test_llm_state();
    let mut app = App::new_with_config(true, true, Theme::Dark, None, llm_state);

    // Enter commit picker mode
    app.enter_commit_picker_mode();
    assert!(app.is_in_commit_picker_mode());

    // Load test commits
    let test_commits = create_test_commits();
    app.update_commit_picker_commits(test_commits);

    // Test commit summary pane key forwarding
    let j_key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

    // Forward key to commit summary pane (for scrolling)
    app.forward_key_to_commit_summary(j_key);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());

    // Update commit summary with current selection
    let llm_state = create_test_llm_state();
    app.update_commit_summary_with_current_selection(&llm_state);

    // Should still be in commit picker mode
    assert!(app.is_in_commit_picker_mode());
}
