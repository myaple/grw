# Implementation Plan

- [x] 1. Create core data structures for commit information
  - Define `CommitInfo`, `CommitFileChange`, and `FileChangeStatus` structs in git.rs
  - Add `AppMode` enum to ui.rs to track Normal vs CommitPicker modes
  - Create `CommitPickerState` struct to manage commit picker internal state
  - _Requirements: 1.1, 2.1, 4.1_

- [x] 2. Extend GitWorker with commit history functionality
  - Add `get_commit_history()` method to GitWorker to fetch commit list with SHA and message
  - Add `get_commit_file_changes()` method to get file modifications for a specific commit
  - Add unit tests for commit history retrieval and file change detection
  - _Requirements: 2.1, 2.2, 5.3_

- [x] 3. Implement CommitPickerPane for displaying commit list
  - Create new `CommitPickerPane` struct implementing the `Pane` trait
  - Implement rendering logic to display commits with short SHA and first line of message
  - Add navigation handling for j/k keys and g+t/g+T combinations
  - Register CommitPickerPane with PaneRegistry in ui.rs
  - _Requirements: 1.2, 3.1, 3.2, 3.3_

- [x] 4. Implement CommitSummaryPane for commit details
  - Create new `CommitSummaryPane` struct implementing the `Pane` trait
  - Implement rendering logic to show modified files with +/- line counts
  - Add placeholder for LLM summary display (initially show "Generating summary...")
  - Add scroll navigation for the file list using existing diff pane navigation patterns
  - _Requirements: 2.1, 2.2, 2.4_

- [x] 5. Add commit picker mode state management to App
  - Add `app_mode: AppMode` and `selected_commit: Option<CommitInfo>` fields to App struct
  - Implement `enter_commit_picker_mode()` and `exit_commit_picker_mode()` methods
  - Add `select_commit()` method to handle commit selection and mode transition
  - Add getter methods for commit picker state access
  - _Requirements: 4.1, 4.2, 4.3, 5.2_

- [x] 6. Integrate Ctrl+P key binding to activate commit picker
  - Add Ctrl+P key handling in main.rs `handle_key_event()` function
  - Implement mode transition logic to switch from normal diff view to commit picker
  - Ensure commit picker only activates when in appropriate diff mode
  - Add unit tests for key binding activation
  - _Requirements: 1.1, 1.3, 5.1_

- [x] 7. Implement commit selection and return to normal mode
  - Add Enter key handling in CommitPickerPane to select highlighted commit
  - Implement commit selection logic that updates App state with chosen commit
  - Add logic to return to normal file browser + diff view with selected commit data
  - Ensure selected commit's files and diffs are properly loaded and displayed
  - _Requirements: 4.1, 4.2, 4.3, 4.4_

- [x] 8. Add LLM integration for commit summaries
  - Extend CommitSummaryPane to request short, 2 sentence LLM summaries of commit changes
  - Integrate with existing LLM client infrastructure from AdvicePane
  - Implement background summary generation that updates the display when complete
  - Add error handling and fallback display when LLM is unavailable
  - _Requirements: 2.2, 2.3, 5.4_

- [x] 9. Implement commit picker layout and pane coordination
  - Update render logic in ui.rs to handle commit picker mode layout
  - Ensure left pane shows CommitPickerPane and right pane shows CommitSummaryPane
  - Add proper pane switching and visibility management for commit picker mode
  - Implement responsive layout that maintains existing UI structure
  - _Requirements: 1.2, 1.3, 5.1, 5.2_

- [x] 10. Add comprehensive error handling and edge cases
  - Handle repositories with no commit history gracefully
  - Add proper error messages when git operations fail
  - Implement loading states for commit history and file change retrieval
  - Add validation for commit selection and mode transitions
  - _Requirements: 5.4, 2.3_

- [x] 11. Write integration tests for commit picker workflow
  - Create tests for complete Ctrl+P -> navigate -> Enter -> return workflow
  - Test commit picker with various repository states (empty, single commit, many commits)
  - Add tests for g+t/g+T navigation and proper commit highlighting
  - Test integration with existing diff navigation after commit selection
  - Ensure tests are in a separate file to manage file lengths
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.4_

- [x] 12. Optimize performance and add caching
  - Implement eager loading for large commit histories (load first n commits, configurable)
  - Optimize rendering performance for large commit lists using proper scroll offsets
  - _Requirements: 5.3, 5.4_

- [x] 13. Enhance help system with commit picker shortcuts
  - Extend existing HelpPane render method to detect commit picker mode
  - Add commit picker keyboard shortcuts (Ctrl+P, Ctrl+W, g+t, g+T, Enter) to help content
  - Update help content to show commit picker shortcuts when in commit picker mode
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 6.5_

- [x] 14. Update documentation with new keyboard shortcuts
  - Update README.md to include Ctrl+W and Ctrl+P keyboard shortcuts
  - Ensure all help documentation includes the new shortcuts
  - Update any other documentation files to maintain consistency
  - _Requirements: 7.1, 7.2, 7.3, 7.4_

- [x] 15. Implement LLM summary caching in GitWorker
  - Add llm_summary_cache HashMap to GitWorker struct
  - Implement get_cached_summary and cache_summary methods
  - Integrate summary cache with existing cache size limits and eviction logic
  - Add cache clearing functionality for summary cache
  - _Requirements: 8.1, 8.2, 8.3, 8.4, 8.5_

- [x] 16. Add separate LLM model configuration
  - Extend LlmConfig struct to support separate advice_model and summary_model fields
  - Update configuration parsing to handle separate model specifications
  - Modify LLM client usage to use appropriate model based on use case (advice vs summary)
  - Implement fallback to default model when specific models are not configured
  - _Requirements: 9.1, 9.2, 9.3, 9.4, 9.5_

- [x] 17. Implement summary pre-loading system
  - Create SummaryPreloader struct with configurable pre-load count (default 5)
  - Implement background summary generation for upcoming commits
  - Add pre-loading logic that triggers when entering commit picker mode
  - Implement continuous pre-loading as user navigates through commits
  - Add error handling for pre-loading failures without blocking UI
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [ ] 18. Update CommitSummaryPane to use cached summaries
  - Modify CommitSummaryPane to check GitWorker cache before generating new summaries
  - Update summary generation to cache results in GitWorker
  - Implement instant display of cached summaries for improved navigation performance
  - Add loading states that differentiate between cached and generating summaries
  - _Requirements: 8.1, 8.2, 8.3_

- [ ] 19. Write comprehensive tests for new caching and help features
  - Add unit tests for LLM summary caching functionality in GitWorker
  - Test help system enhancements with commit picker mode detection
  - Add integration tests for summary pre-loading system
  - Test separate LLM model configuration and fallback behavior
  - Add performance tests for cached vs non-cached summary retrieval
  - _Requirements: 6.1, 8.1, 9.1, 10.1_