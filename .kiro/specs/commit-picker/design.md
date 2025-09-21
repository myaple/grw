# Design Document

## Overview

The commit picker feature adds a new interface mode to the TUI application that allows users to browse and select specific commits for analysis. This feature integrates with the existing pane system and maintains consistency with the current navigation patterns while providing enhanced commit exploration capabilities.

## Architecture

### Core Components

1. **CommitPickerPane** - A new pane that displays the list of commits
2. **CommitSummaryPane** - A new pane that shows commit details and file changes
3. **CommitPickerMode** - A new application mode that coordinates the commit picker interface
4. **CommitData** - Data structures to represent commit information
5. **GitCommitService** - Service layer for fetching commit data
6. **Enhanced HelpPane** - Extend existing help system to include commit picker shortcuts
7. **LLM Summary Cache** - Extend existing GitWorker cache for LLM summaries
8. **LLMConfig** - Configuration for separate advice and summary models
9. **SummaryPreloader** - Background service for pre-generating commit summaries

### Integration Points

- **App State Management** - Extends existing `App` struct with commit picker state
- **Pane Registry** - Registers new panes with the existing pane system
- **Key Handler** - Integrates with existing key handling in `main.rs`
- **Git Integration** - Extends `GitRepo` and `GitWorker` for commit operations

## Components and Interfaces

### Data Models

```rust
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub sha: String,
    pub short_sha: String,
    pub message: String,
    pub author: String,
    pub date: String,
    pub files_changed: Vec<CommitFileChange>,
}

#[derive(Debug, Clone)]
pub struct CommitFileChange {
    pub path: PathBuf,
    pub status: FileChangeStatus,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone)]
pub enum FileChangeStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
}

// Remove AppMode enum - use existing boolean flags pattern
// App already has commit_picker_mode: bool pattern established

#[derive(Debug, Clone)]
pub struct LLMConfig {
    pub advice_model: String,
    pub summary_model: String,
    pub default_model: String,
}

#[derive(Debug, Clone)]
pub struct SummaryCacheEntry {
    pub commit_sha: String,
    pub summary: String,
    pub timestamp: SystemTime,
}

#[derive(Debug, Clone)]
pub struct PreloadConfig {
    pub enabled: bool,
    pub count: usize, // Default: 5
}
```

### CommitPickerPane

```rust
pub struct CommitPickerPane {
    visible: bool,
    commits: Vec<CommitInfo>,
    current_index: usize,
    scroll_offset: usize,
}

impl Pane for CommitPickerPane {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, git_repo: &GitRepo);
    fn handle_event(&mut self, event: &AppEvent) -> bool;
    // Navigation: j/k, g+t/g+T for next/prev commit
}
```

### CommitSummaryPane

```rust
pub struct CommitSummaryPane {
    visible: bool,
    current_commit: Option<CommitInfo>,
    scroll_offset: usize,
    llm_summary: Option<String>,
    // Remove separate cache - use GitWorker's existing cache system
}

impl Pane for CommitSummaryPane {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, git_repo: &GitRepo);
    fn handle_event(&mut self, event: &AppEvent) -> bool;
    // Shows file list with +/- counts and cached/generated LLM summary
    // Check GitWorker cache first before generating new summaries
}
```

### Enhanced HelpPane

```rust
// Extend existing HelpPane to include commit picker shortcuts
impl Pane for HelpPane {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, git_repo: &GitRepo);
    // Enhanced to detect commit picker mode and show relevant shortcuts
    // Add Ctrl+P, Ctrl+W, g+t, g+T, Enter shortcuts when in commit picker mode
}
```

### LLM Summary Cache (GitWorker Extension)

```rust
// Extend existing GitWorker cache system
impl GitWorker {
    // Add to existing cache fields:
    // llm_summary_cache: HashMap<String, String>,
    
    pub fn get_cached_summary(&self, commit_sha: &str) -> Option<String>;
    pub fn cache_summary(&mut self, commit_sha: String, summary: String);
    pub fn clear_summary_cache(&mut self);
    
    // Reuse existing cache management with set_cache_size()
    // Apply same eviction logic as existing commit caches
}
```

### SummaryPreloader

```rust
pub struct SummaryPreloader {
    llm_client: Option<LlmClient>,
    config: PreloadConfig,
    active_tasks: HashSet<String>,
    // Use GitWorker's existing cache through message passing
}

impl SummaryPreloader {
    pub fn preload_summaries(&mut self, commits: &[CommitInfo], git_worker_tx: &mpsc::Sender<GitWorkerMessage>);
    pub fn preload_around_index(&mut self, commits: &[CommitInfo], current_index: usize, git_worker_tx: &mpsc::Sender<GitWorkerMessage>);
    pub fn is_loading(&self, commit_sha: &str) -> bool;
}
```

### App State Extensions

```rust
impl App {
    pub fn enter_commit_picker_mode(&mut self);
    pub fn exit_commit_picker_mode(&mut self);
    pub fn is_in_commit_picker_mode(&self) -> bool;
    pub fn select_commit(&mut self, commit: CommitInfo);
    pub fn get_current_commit(&self) -> Option<&CommitInfo>;
    pub fn get_llm_config(&self) -> &LLMConfig;
    
    // New fields added to App struct:
    // commit_picker_mode: bool, // Reuse existing pattern instead of enum
    // selected_commit: Option<CommitInfo>,
    // commit_picker_state: CommitPickerState,
    // summary_preloader: SummaryPreloader,
    // llm_config: LLMConfig,
    
    // Reuse existing help system - no changes needed to help mode management
}
```

### Git Service Extensions

```rust
impl GitWorker {
    fn get_commit_history(&self, limit: usize) -> Result<Vec<CommitInfo>>;
    fn get_commit_diff(&self, commit_sha: &str) -> Result<Vec<FileDiff>>;
    fn get_commit_file_changes(&self, commit_sha: &str) -> Result<Vec<CommitFileChange>>;
}
```

## Data Models

### Commit Information Structure
- **CommitInfo**: Contains SHA, message, author, date, and file changes
- **CommitFileChange**: Represents individual file modifications within a commit
- **FileChangeStatus**: Enum for different types of file changes (Added, Modified, Deleted, Renamed)

### State Management
- **AppMode**: Enum to track whether the app is in Normal or CommitPicker mode
- **CommitPickerState**: Struct containing current commit selection, scroll position, and loaded commits
- **Selected Commit**: Optional commit that becomes the new "current commit" for diff analysis

## Error Handling

### Git Operation Errors
- Handle cases where git history cannot be retrieved
- Graceful degradation when commit details are unavailable
- Error messages displayed in the commit picker pane

### LLM Integration Errors
- Handle cases where LLM summary generation fails
- Show loading states and fallback to basic file change information
- Timeout handling for LLM requests
- Cache corruption or invalid entries handling
- Model configuration errors (invalid model names, network issues)

### Navigation Edge Cases
- Handle empty commit history
- Manage navigation when no commits are available
- Proper state restoration when exiting commit picker mode
- Help mode overlay state management
- Cache memory pressure and eviction edge cases

### Caching Errors
- Leverage existing GitWorker cache management and eviction policies
- Handle LLM summary cache misses gracefully by generating new summaries
- Reuse existing cache size limits and memory management from GitWorker
- Handle concurrent access through existing GitWorker message passing system

## Testing Strategy

### Unit Tests
- **CommitPickerPane**: Test rendering with various commit lists, navigation behavior
- **CommitSummaryPane**: Test display of commit details and file changes
- **GitCommitService**: Test commit history retrieval and diff generation
- **App State Management**: Test mode transitions and commit selection

### Integration Tests
- **Key Navigation**: Test g+t/g+T navigation in commit picker mode
- **Mode Transitions**: Test Ctrl+P activation and Enter selection
- **Pane Coordination**: Test left/right pane synchronization
- **Git Integration**: Test with real git repositories

### Manual Testing Scenarios
- Large commit histories (performance testing)
- Commits with many file changes
- Merge commits and complex diffs
- Repositories with no commit history
- Network/LLM timeout scenarios

## Implementation Notes

### Pane System Integration
The commit picker leverages the existing pane registry system by:
1. Registering `CommitPickerPane` and `CommitSummaryPane` with `PaneRegistry`
2. Using existing `AppEvent` system for key handling
3. Following established patterns for pane visibility and rendering

### Key Binding Strategy
- Ctrl+P activates commit picker mode (handled in main.rs key handler)
- g+t/g+T navigation reuses existing timing logic from App::handle_g_press()
- Enter key selection handled by CommitPickerPane
- Other navigation keys (j/k, arrows) follow existing patterns

### Performance Considerations
- Lazy loading of commit history (initial load of 50-100 commits)
- Caching of commit file changes to avoid repeated git operations
- Efficient rendering for large commit lists using scroll offsets
- Background LLM summary generation to avoid blocking UI

### LLM Integration
- Reuse existing LLM client infrastructure from AdvicePane
- Generate summaries based on commit diff content using configured summary model
- Generate advice using configured advice model
- Cache summaries to avoid repeated API calls with configurable eviction policies
- Provide fallback display when LLM is unavailable
- Pre-generate summaries in background for smooth navigation experience

### Configuration Management
- Support separate model configuration for advice vs summary generation
- Provide sensible defaults when specific models are not configured
- Allow runtime configuration updates without application restart
- Support configurable cache size and eviction policies
- Configurable pre-loading count with default of 5 commits

### State Persistence
- Remember last selected commit when returning to normal mode
- Preserve scroll positions when switching between modes
- Maintain commit picker state across theme changes and window resizes
- Persist summary cache across application sessions (optional)
- Maintain help mode overlay state and proper mode restoration

### Help System Architecture
- Extend existing HelpPane render method to detect commit picker mode
- Add commit picker shortcuts to existing help content structure
- Maintain existing help toggle behavior and overlay system
- Update README and help documentation to include Ctrl+W and Ctrl+P shortcuts
- Leverage existing ActivePane enum to determine context-specific help content