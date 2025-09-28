# Design Document

## Overview

The pane module refactoring will transform a monolithic 3262-line mod.rs file into a well-organized module structure where each pane implementation lives in its own file. This design maintains backward compatibility while significantly improving code maintainability and developer experience.

## Architecture

### Current Structure
```
src/pane/
└── mod.rs (3262 lines containing all panes)
```

### Target Structure
```
src/pane/
├── mod.rs (module declarations and re-exports)
├── advice_panel.rs
├── file_tree_pane.rs
├── monitor_pane.rs
├── diff_pane.rs
├── side_by_side_diff_pane.rs
├── help_pane.rs
├── status_bar_pane.rs
├── commit_picker_pane.rs
└── commit_summary_pane.rs
```

## Components and Interfaces

### Core Components to Remain in mod.rs

1. **Pane Trait**: The core trait that all panes implement
2. **PaneId Enum**: Identifier enum for different pane types
3. **AppEvent Enum**: Event system for pane communication
4. **PaneRegistry**: Central registry for managing panes
5. **Shared Data Structures**: Types used across multiple panes

### Individual Pane Files

Each pane file will contain:
- The pane struct definition
- Associated enums/structs specific to that pane
- All impl blocks for the pane
- Private helper functions
- Necessary imports

### Pane-Specific Components

1. **AdvicePanel** (`advice_panel.rs`)
   - AdviceMode enum
   - MessageRole enum  
   - ImprovementPriority enum
   - AdviceImprovement struct
   - ChatMessageData struct
   - AdviceContent enum
   - LoadingState enum

2. **FileTreePane** (`file_tree_pane.rs`)
   - Simple pane with minimal dependencies

3. **MonitorPane** (`monitor_pane.rs`)
   - Simple pane with scroll functionality

4. **DiffPane** (`diff_pane.rs`)
   - Basic diff display functionality

5. **SideBySideDiffPane** (`side_by_side_diff_pane.rs`)
   - Enhanced diff display with side-by-side view

6. **HelpPane** (`help_pane.rs`)
   - Static help content display

7. **StatusBarPane** (`status_bar_pane.rs`)
   - Status information display

8. **CommitPickerPane** (`commit_picker_pane.rs`)
   - Git commit selection functionality

9. **CommitSummaryPane** (`commit_summary_pane.rs`)
   - Detailed commit information display

## Data Models

### Import Strategy

Each pane file will import only what it needs:
```rust
// Common imports for most panes
use crate::git::GitRepo;
use crate::ui::{App, Theme};
use ratatui::{Frame, layout::Rect};
use super::{Pane, AppEvent}; // Import from parent module

// Pane-specific imports as needed
```

### Re-export Strategy in mod.rs

```rust
// Module declarations
mod advice_panel;
mod file_tree_pane;
mod monitor_pane;
// ... etc

// Re-exports to maintain public API
pub use advice_panel::*;
pub use file_tree_pane::*;
pub use monitor_pane::*;
// ... etc

// Core types remain in mod.rs
pub trait Pane { /* ... */ }
pub enum PaneId { /* ... */ }
pub enum AppEvent { /* ... */ }
pub struct PaneRegistry { /* ... */ }
```

## Error Handling

### Compilation Safety
- Each extracted file must compile independently
- All necessary imports must be included
- No circular dependencies between pane files

### Runtime Behavior
- Existing error handling patterns will be preserved
- No changes to error propagation or handling logic

## Testing Strategy

### Validation Approach
1. **Compilation Test**: Ensure all files compile without errors
2. **API Compatibility Test**: Verify existing imports continue to work
3. **Functionality Test**: Run existing tests to ensure behavior is unchanged
4. **Integration Test**: Verify PaneRegistry continues to work with all panes

### Test Execution Plan
1. Extract one pane at a time
2. Compile and test after each extraction
3. Verify public API remains intact
4. Run full test suite after completion

## Implementation Phases

### Phase 1: Preparation
- Analyze dependencies between panes
- Identify shared types and functions
- Plan extraction order to minimize dependencies

### Phase 2: Core Infrastructure
- Update mod.rs with module declarations
- Set up re-export structure
- Ensure shared types remain accessible

### Phase 3: Pane Extraction
- Extract panes in dependency order (least dependent first)
- Start with simple panes (HelpPane, StatusBarPane)
- Progress to complex panes (AdvicePanel, CommitSummaryPane)

### Phase 4: Validation
- Compile entire project
- Run tests
- Verify API compatibility
- Clean up any remaining issues

## Migration Strategy

### Dependency Order
Based on analysis, the extraction order should be:
1. HelpPane (minimal dependencies)
2. StatusBarPane (minimal dependencies)  
3. FileTreePane (basic functionality)
4. MonitorPane (basic functionality)
5. DiffPane (basic functionality)
6. SideBySideDiffPane (extends diff functionality)
7. CommitPickerPane (git integration)
8. CommitSummaryPane (complex with LLM integration)
9. AdvicePanel (most complex with async tasks)

### Rollback Plan
- Keep original mod.rs as backup until refactoring is complete
- Each extraction can be reverted independently if issues arise
- Git commits after each successful pane extraction