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