# Implementation Plan

- [x] 1. Prepare mod.rs for refactoring
  - Create backup of original mod.rs file
  - Analyze current imports and dependencies in mod.rs
  - Identify all shared types that need to remain in mod.rs
  - _Requirements: 2.1, 3.1, 3.2_

- [ ] 2. Set up module infrastructure
  - [x] 2.1 Create module declarations in mod.rs
    - Add mod declarations for all pane modules
    - Set up re-export structure using pub use statements
    - Ensure Pane trait, PaneId, AppEvent remain in mod.rs
    - _Requirements: 2.1, 2.2, 3.2, 3.3_

  - [x] 2.2 Extract shared types and keep in mod.rs
    - Keep PaneRegistry struct and implementation in mod.rs
    - Ensure all shared enums and data structures remain accessible
    - Verify imports are properly organized
    - _Requirements: 3.1, 3.2, 3.3_

- [ ] 3. Extract simple panes first
  - [x] 3.1 Extract HelpPane to help_pane.rs
    - Create src/pane/help_pane.rs file
    - Move HelpPane struct and all its implementations
    - Add necessary imports (Pane trait, Frame, Rect, etc.)
    - Test compilation after extraction
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

  - [x] 3.2 Extract StatusBarPane to status_bar_pane.rs
    - Create src/pane/status_bar_pane.rs file
    - Move StatusBarPane struct and implementations
    - Add required imports for rendering and event handling
    - Verify compilation and functionality
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

- [ ] 4. Extract basic functionality panes
  - [x] 4.1 Extract FileTreePane to file_tree_pane.rs
    - Create src/pane/file_tree_pane.rs file
    - Move FileTreePane struct and all associated code
    - Include scroll functionality and tree rendering logic
    - Add imports for List, ListItem, and styling components
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

  - [x] 4.2 Extract MonitorPane to monitor_pane.rs
    - Create src/pane/monitor_pane.rs file
    - Move MonitorPane struct and scroll functionality
    - Include all monitoring display logic
    - Test scroll behavior after extraction
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

- [ ] 5. Extract diff-related panes
  - [x] 5.1 Extract DiffPane to diff_pane.rs
    - Create src/pane/diff_pane.rs file
    - Move DiffPane struct and diff rendering logic
    - Include all diff display functionality
    - Add imports for diff processing and display
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

  - [x] 5.2 Extract SideBySideDiffPane to side_by_side_diff_pane.rs
    - Create src/pane/side_by_side_diff_pane.rs file
    - Move SideBySideDiffPane struct and side-by-side rendering
    - Include enhanced diff display logic
    - Verify side-by-side layout functionality
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

- [ ] 6. Extract git-related panes
  - [x] 6.1 Extract CommitPickerPane to commit_picker_pane.rs
    - Create src/pane/commit_picker_pane.rs file
    - Move CommitPickerPane struct and commit selection logic
    - Include git integration and commit list functionality
    - Add imports for git types and commit handling
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

- [ ] 7. Extract complex panes with external dependencies
  - [x] 7.1 Extract CommitSummaryPane to commit_summary_pane.rs
    - Create src/pane/commit_summary_pane.rs file
    - Move CommitSummaryPane struct and LLM integration code
    - Include commit detail display and summary generation
    - Add imports for LLM client and shared state
    - _Requirements: 1.1, 1.3, 4.1, 4.2_

  - [x] 7.2 Extract AdvicePanel to advice_panel.rs
    - Create src/pane/advice_panel.rs file
    - Move AdvicePanel struct and all associated enums/structs
    - Include AdviceMode, MessageRole, ImprovementPriority enums
    - Move AdviceImprovement, ChatMessageData, AdviceContent types
    - Include LoadingState enum and all async task handling
    - Add comprehensive imports for async, LLM, and UI components
    - _Requirements: 1.1, 1.2, 1.3, 4.1, 4.2_

- [ ] 8. Update mod.rs to final state
  - [x] 8.1 Clean up mod.rs after all extractions
    - Remove all extracted pane implementations from mod.rs
    - Keep only shared types, traits, and PaneRegistry
    - Ensure all pub use statements are correct
    - Verify module declarations are complete
    - _Requirements: 2.1, 2.2, 3.1, 4.3_

  - [x] 8.2 Optimize imports and organization
    - Clean up unused imports in mod.rs
    - Organize remaining code logically
    - Add module-level documentation if needed
    - Ensure consistent code formatting
    - _Requirements: 4.2, 4.3, 4.4_

- [ ] 9. Validation and testing
  - [x] 9.1 Compile and test entire project
    - Run cargo check to verify compilation
    - Run cargo test to ensure all tests pass
    - Verify no functionality regressions
    - Test PaneRegistry with all extracted panes
    - _Requirements: 2.1, 2.3, 2.4_

  - [x] 9.2 Verify public API compatibility
    - Test that external imports still work
    - Verify all pane types are accessible
    - Ensure PaneRegistry can create all panes
    - Confirm no breaking changes to public interface
    - _Requirements: 2.1, 2.2, 2.3, 2.4_