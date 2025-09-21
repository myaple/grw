# Requirements Document

## Introduction

This feature adds a commit picker interface to the TUI application, allowing users to browse and select specific commits to analyze. The commit picker replaces the file list pane with a commit selector showing commit metadata, while the right pane displays commit summary information. Once a commit is selected, the interface returns to the normal file browser + diff view for analyzing the chosen commit.

## Requirements

### Requirement 1

**User Story:** As a developer, I want to access a commit picker interface, so that I can browse and select specific commits to analyze instead of being limited to dirty/staged/previous commit.

#### Acceptance Criteria

1. WHEN the user presses Ctrl+P in diff mode THEN the system SHALL switch to commit picker mode
2. WHEN commit picker mode is activated THEN the left pane SHALL display a list of commits with short SHA and first line of commit message
3. WHEN commit picker mode is activated THEN the file list pane SHALL be replaced with the commit selector interface

### Requirement 2

**User Story:** As a developer, I want to see commit metadata and summaries, so that I can understand what each commit contains before selecting it.

#### Acceptance Criteria

1. WHEN a commit is highlighted in the commit picker THEN the right pane SHALL display a list of modified files for that commit
2. WHEN displaying modified files THEN the system SHALL show the number of lines added and subtracted for each file
3. WHEN displaying modified files THEN the system SHALL show an LLM-generated summary of what the commit has changed
4. WHEN the commit picker is active THEN the right pane SHALL use the same navigation hotkeys as the diff pane

### Requirement 3

**User Story:** As a developer, I want to navigate through commits efficiently, so that I can quickly find the commit I'm interested in analyzing.

#### Acceptance Criteria

1. WHEN the user presses 'g' then 't' in commit picker mode THEN the system SHALL move to the next commit in the list
2. WHEN the user presses 'g' then 'T' in commit picker mode THEN the system SHALL move to the previous commit in the list
3. WHEN the user uses other navigation hotkeys THEN the system SHALL behave the same as the file tree window
4. WHEN the user presses up/down arrows THEN the system SHALL navigate between commits and update the right pane accordingly

### Requirement 4

**User Story:** As a developer, I want to select a commit and return to normal analysis mode, so that I can examine the specific commit's changes in detail.

#### Acceptance Criteria

1. WHEN the user presses Enter on a selected commit THEN the system SHALL exit commit picker mode
2. WHEN exiting commit picker mode THEN the system SHALL return to the normal file browser + diff interface
3. WHEN returning to normal mode THEN the system SHALL display the selected commit's changes instead of dirty/staged/previous commit
4. WHEN in normal mode after commit selection THEN all existing diff navigation features SHALL work with the selected commit

### Requirement 5

**User Story:** As a developer, I want the commit picker to integrate seamlessly with existing functionality, so that the interface remains consistent and intuitive.

#### Acceptance Criteria

1. WHEN the commit picker is active THEN the system SHALL maintain the same visual layout structure as the existing interface
2. WHEN switching between modes THEN the system SHALL preserve the current state where appropriate
3. WHEN the commit picker displays commits THEN the system SHALL use a consistent ordering (most recent first)
4. WHEN no commits are available THEN the system SHALL display an appropriate message and handle the edge case gracefully

### Requirement 6

**User Story:** As a developer, I want access to a dynamic help page that documents all keyboard shortcuts, so that I can quickly reference available commands without leaving the application.

#### Acceptance Criteria

1. WHEN the user presses '?' or F1 in any mode THEN the system SHALL display a dynamic help overlay
2. WHEN the help overlay is displayed THEN the system SHALL show all available keyboard shortcuts for the current mode
3. WHEN in commit picker mode THEN the help SHALL include Ctrl+P, Ctrl+W, g+t, g+T, Enter, ctrl + c, q, and navigation keys
4. WHEN the help overlay is active THEN the user SHALL be able to press Esc or '?' to close it
5. WHEN the help overlay is displayed THEN it SHALL overlay the current interface without changing the underlying state

### Requirement 7

**User Story:** As a developer, I want all documentation to be complete and up-to-date, so that I can reference the correct keyboard shortcuts and functionality.

#### Acceptance Criteria

1. WHEN the README is updated THEN it SHALL include documentation for Ctrl+W and Ctrl+P keyboard shortcuts
2. WHEN help documentation exists THEN it SHALL include all current keyboard shortcuts including Ctrl+W and Ctrl+P
3. WHEN new keyboard shortcuts are added THEN all help pages SHALL be updated to reflect the changes
4. WHEN documentation is provided THEN it SHALL be consistent across all help sources (in-app help, README, other docs)

### Requirement 8

**User Story:** As a developer, I want LLM-generated commit summaries to be cached, so that I can navigate through commits quickly without waiting for regeneration of previously viewed summaries.

#### Acceptance Criteria

1. WHEN an LLM summary is generated for a commit THEN the system SHALL cache the summary for future use
2. WHEN navigating to a previously viewed commit THEN the system SHALL display the cached summary instantly
3. WHEN the cache contains a summary for a commit THEN the system SHALL NOT regenerate the summary
4. WHEN the application restarts THEN cached summaries SHALL be cleared
5. WHEN memory usage becomes high THEN the system SHALL implement cache eviction policies to manage memory

### Requirement 9

**User Story:** As a developer, I want to configure different LLM models for advice and summary generation, so that I can optimize performance and cost for different use cases.

#### Acceptance Criteria

1. WHEN configuring the application THEN the system SHALL allow separate model specification for advice generation
2. WHEN configuring the application THEN the system SHALL allow separate model specification for commit summary generation
3. WHEN generating advice THEN the system SHALL use the configured advice model
4. WHEN generating commit summaries THEN the system SHALL use the configured summary model
5. WHEN no specific model is configured THEN the system SHALL fall back to a default model for both use cases

### Requirement 10

**User Story:** As a developer, I want commit summaries to be pre-generated for upcoming commits, so that I have instant access to summaries as I navigate through the commit history.

#### Acceptance Criteria

1. WHEN entering commit picker mode THEN the system SHALL pre-generate summaries for a configurable number of commits
2. WHEN the pre-population count is not configured THEN the system SHALL default to pre-generating 5 commit summaries
3. WHEN navigating through commits THEN the system SHALL continue pre-generating summaries for upcoming commits
4. WHEN pre-generating summaries THEN the system SHALL not block the user interface
5. WHEN pre-generation fails for a commit THEN the system SHALL continue with other commits and handle the error gracefully