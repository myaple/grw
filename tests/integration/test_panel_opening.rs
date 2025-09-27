#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{PaneRegistry, PaneId, AdvicePanel};
    use crate::config::{Config, AdviceConfig};
    use crate::shared_state::SharedState;

    #[test]
    fn test_panel_opening_with_ctrl_l() {
        // Test the complete panel opening flow when Ctrl+L is pressed
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        // This test establishes the contract for the panel opening flow
        // The actual implementation will need to handle key events, panel creation, etc.

        // Step 1: Verify initial state - no advice panel should be active
        let registry = PaneRegistry::new();
        let active_panes = registry.get_active_panes();
        assert!(!active_panes.contains(&PaneId::Advice),
                "Advice panel should not be active initially");

        // Step 2: Simulate Ctrl+L key press
        // This should trigger the advice panel opening logic
        let key_event = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('l'),
            crossterm::event::KeyModifiers::CONTROL,
        );

        // The key handling logic should create and show the advice panel
        // This will fail until the key handling is implemented
        let result = std::panic::catch_unwind(|| {
            // This represents the key handling function that should be implemented
            handle_ctrl_l_key(key_event, &shared_state)
        });

        // Should not panic when handling the key event
        assert!(result.is_ok(), "Ctrl+L key handling should not panic");

        // Step 3: Verify advice panel is now active
        // This will fail until the panel opening logic is implemented
        let updated_panes = registry.get_active_panes();
        assert!(updated_panes.contains(&PaneId::Advice),
                "Advice panel should be active after Ctrl+L");
    }

    #[test]
    fn test_panel_opening_with_git_diff_analysis() {
        // Test that opening the panel automatically analyzes git diff
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        // Create a mock git repository with some changes
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path();

        // Initialize git repo and create some sample changes
        std::fs::write(repo_path.join("test.rs"), "fn main() { println!(\"old\"); }").unwrap();

        let result = std::panic::catch_unwind(|| {
            // Simulate opening advice panel with git diff context
            open_advice_panel_with_git_context(repo_path, &shared_state)
        });

        assert!(result.is_ok(), "Opening advice panel with git context should not panic");

        // Verify that the panel attempts to analyze the git diff
        // This establishes the contract that diff analysis should happen automatically
        let analysis_result = result.unwrap();
        assert!(analysis_result.is_ok(), "Git diff analysis should be attempted");
    }

    #[test]
    fn test_panel_opening_with_specific_commit() {
        // Test panel opening when a specific commit is selected
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        // Simulate selecting a specific commit before opening advice panel
        let commit_hash = "abc123def456";

        let result = std::panic::catch_unwind(|| {
            open_advice_panel_for_commit(commit_hash, &shared_state)
        });

        assert!(result.is_ok(), "Opening advice panel for specific commit should not panic");

        // Verify that the panel analyzes the specific commit
        let panel_result = result.unwrap();
        assert!(panel_result.is_ok(), "Should be able to open panel for specific commit");
    }

    #[test]
    fn test_panel_opening_error_handling() {
        // Test error handling when opening the panel fails
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        // Test opening panel in non-git directory
        let temp_dir = tempfile::tempdir().unwrap();
        let non_git_dir = temp_dir.path();

        let result = std::panic::catch_unwind(|| {
            open_advice_panel_with_git_context(non_git_dir, &shared_state)
        });

        assert!(result.is_ok(), "Should handle non-git directory gracefully");

        let panel_result = result.unwrap();
        // Should either succeed (with empty analysis) or provide a clear error
        assert!(panel_result.is_ok(), "Should handle non-git directory without crashing");
    }

    #[test]
    fn test_panel_toggle_behavior() {
        // Test that Ctrl+L toggles the panel (opens when closed, closes when open)
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        let ctrl_l_key = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('l'),
            crossterm::event::KeyModifiers::CONTROL,
        );

        // Initially closed
        assert!(!is_advice_panel_active(&shared_state), "Panel should start closed");

        // First Ctrl+L should open
        let result1 = handle_ctrl_l_key(ctrl_l_key, &shared_state);
        assert!(result1.is_ok(), "First Ctrl+L should succeed");
        assert!(is_advice_panel_active(&shared_state), "Panel should be open after first Ctrl+L");

        // Second Ctrl+L should close
        let result2 = handle_ctrl_l_key(ctrl_l_key, &shared_state);
        assert!(result2.is_ok(), "Second Ctrl+L should succeed");
        assert!(!is_advice_panel_active(&shared_state), "Panel should be closed after second Ctrl+L");
    }

    #[test]
    fn test_panel_opening_performance() {
        // Test that panel opening meets performance requirements
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        let start_time = std::time::Instant::now();

        let result = std::panic::catch_unwind(|| {
            // Simulate panel opening
            open_advice_panel_with_git_context(std::path::Path::new("."), &shared_state)
        });

        let elapsed = start_time.elapsed();

        assert!(result.is_ok(), "Panel opening should not panic");
        assert!(elapsed.as_millis() < 100, "Panel opening should complete within 100ms");

        let panel_result = result.unwrap();
        assert!(panel_result.is_ok(), "Panel opening should succeed");
    }

    #[test]
    fn test_panel_opening_with_configuration() {
        // Test that panel opening respects configuration settings
        let mut config = Config::default();
        config.advice = Some(AdviceConfig {
            enabled: Some(false), // Panel disabled in config
            ..Default::default()
        });

        let shared_state = SharedState::new(config.clone());

        let result = std::panic::catch_unwind(|| {
            open_advice_panel_with_git_context(std::path::Path::new("."), &shared_state)
        });

        assert!(result.is_ok(), "Should handle disabled config gracefully");

        let panel_result = result.unwrap();
        // When disabled, the panel might still open but with limited functionality
        // This establishes the contract for disabled configuration handling
        assert!(panel_result.is_ok(), "Should handle disabled configuration");
    }

    #[test]
    fn test_panel_opening_state_persistence() {
        // Test that panel state persists correctly during opening
        let config = Config::default();
        let shared_state = SharedState::new(config.clone());

        // Open panel
        let result1 = std::panic::catch_unwind(|| {
            open_advice_panel_with_git_context(std::path::Path::new("."), &shared_state)
        });

        assert!(result1.is_ok(), "First panel opening should succeed");

        // Check that state is properly initialized
        let panel_state = get_advice_panel_state(&shared_state);
        assert!(panel_state.is_some(), "Panel state should be accessible");

        // Close panel
        let result2 = std::panic::catch_unwind(|| {
            close_advice_panel(&shared_state)
        });

        assert!(result2.is_ok(), "Panel closing should succeed");

        // Check that state is properly cleaned up
        let cleaned_state = get_advice_panel_state(&shared_state);
        // This establishes the contract for state management during panel operations
        assert!(cleaned_state.is_some(), "State management should be consistent");
    }

    // Helper functions that represent the implementation contracts
    // These will fail until the actual implementation is provided
    fn handle_ctrl_l_key(_key: crossterm::event::KeyEvent, _state: &SharedState) -> Result<(), String> {
        // This should be implemented to handle Ctrl+L key events
        Err("Not implemented yet".to_string())
    }

    fn open_advice_panel_with_git_context(_path: &std::path::Path, _state: &SharedState) -> Result<(), String> {
        // This should be implemented to open advice panel with git diff analysis
        Err("Not implemented yet".to_string())
    }

    fn open_advice_panel_for_commit(_commit: &str, _state: &SharedState) -> Result<(), String> {
        // This should be implemented to open advice panel for specific commit
        Err("Not implemented yet".to_string())
    }

    fn is_advice_panel_active(_state: &SharedState) -> bool {
        // This should be implemented to check if advice panel is active
        false
    }

    fn close_advice_panel(_state: &SharedState) -> Result<(), String> {
        // This should be implemented to close the advice panel
        Err("Not implemented yet".to_string())
    }

    fn get_advice_panel_state(_state: &SharedState) -> Option<String> {
        // This should be implemented to get panel state
        None
    }
}