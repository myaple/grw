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

- [ ] 8. Add LLM integration for commit summaries
  - Extend CommitSummaryPane to request LLM summaries of commit changes
  - Integrate with existing LLM client infrastructure from AdvicePane
  - Implement background summary generation that updates the display when complete
  - Add error handling and fallback display when LLM is unavailable
  - _Requirements: 2.2, 2.3, 5.4_

- [ ] 9. Implement commit picker layout and pane coordination
  - Update render logic in ui.rs to handle commit picker mode layout
  - Ensure left pane shows CommitPickerPane and right pane shows CommitSummaryPane
  - Add proper pane switching and visibility management for commit picker mode
  - Implement responsive layout that maintains existing UI structure
  - _Requirements: 1.2, 1.3, 5.1, 5.2_

- [ ] 10. Add comprehensive error handling and edge cases
  - Handle repositories with no commit history gracefully
  - Add proper error messages when git operations fail
  - Implement loading states for commit history and file change retrieval
  - Add validation for commit selection and mode transitions
  - _Requirements: 5.4, 2.3_

- [ ] 11. Write integration tests for commit picker workflow
  - Create tests for complete Ctrl+P -> navigate -> Enter -> return workflow
  - Test commit picker with various repository states (empty, single commit, many commits)
  - Add tests for g+t/g+T navigation and proper commit highlighting
  - Test integration with existing diff navigation after commit selection
  - _Requirements: 3.1, 3.2, 3.3, 3.4, 4.4_

- [ ] 12. Optimize performance and add caching
  - Implement lazy loading for large commit histories (load first 50-100 commits)
  - Add caching for commit file changes to avoid repeated git operations
  - Optimize rendering performance for large commit lists using proper scroll offsets
  - Add background loading indicators for slow git operations
  - _Requirements: 5.3, 5.4_