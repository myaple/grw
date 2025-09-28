# GRW Spring Cleaning Plan

## Overview
This document outlines a comprehensive cleanup and refactoring plan for the GRW (Git Repository Watcher) project. The goal is to improve maintainability, reduce code complexity, and create a more modular architecture.

## Current State Analysis

### Codebase Size
- **Total modules**: 11 modules
- **Total lines**: ~10,000 lines of code
- **Largest modules**: pane.rs (3,353 lines), ui.rs (2,020 lines), shared_state.rs (1,755 lines)

### Key Issues Identified
1. **Excessive dead code annotations**: Nearly every module has `#[allow(dead_code)]`
2. **Monolithic modules**: Several modules violate single responsibility principle
3. **Complex main.rs**: 691 lines with complex event handling
4. **Mixed concerns**: UI logic mixed with business logic in several modules

## Cleanup Tasks

### Phase 1: Dead Code Cleanup (Critical)

#### 1.1 Remove All `#[allow(dead_code)]` Annotations
- **Priority**: Critical
- **Files affected**: ui.rs, shared_state.rs, monitor.rs, git_worker.rs, pane.rs, git.rs, config.rs
- **Action**: Remove all `#[allow(dead_code)]` annotations and actually remove unused code
- **Expected impact**: Reduce codebase by ~20-30%

#### 1.2 Audit and Remove Unused Imports
- **Priority**: High
- **Action**: Remove unused imports across all modules
- **Tool**: Use `cargo clippy` to identify unused imports

#### 1.3 Remove Unused Dependencies
- **Priority**: Medium
- **Action**: Audit Cargo.toml for unused dependencies
- **Tool**: Use `cargo machete` or manual audit

### Phase 2: Module Splitting (High Priority)

#### 2.1 Split pane.rs (3,353 lines)
- **Priority**: High
- **Target**: Split into focused modules
- **New structure**:
  - `src/pane/mod.rs` - Core trait definitions and common functionality
  - `src/pane/file_tree.rs` - File tree pane implementation
  - `src/pane/monitor.rs` - Monitor pane implementation
  - `src/pane/diff.rs` - Diff pane implementation
  - `src/pane/commit_picker.rs` - Commit picker pane implementation
  - `src/pane/advice.rs` - Advice panel implementation

#### 2.2 Split ui.rs (2,020 lines)
- **Priority**: High
- **Target**: Split into focused modules
- **New structure**:
  - `src/ui/mod.rs` - Core UI components and traits
  - `src/ui/app.rs` - Application state management
  - `src/ui/render.rs` - Rendering logic and drawing
  - `src/ui/theme.rs` - Theme management and styling

#### 2.3 Split shared_state.rs (1,755 lines)
- **Priority**: Medium
- **Target**: Split by domain
- **New structure**:
  - `src/shared_state/mod.rs` - Core manager and common functionality
  - `src/shared_state/git.rs` - Git-specific state management
  - `src/shared_state/llm.rs` - LLM-specific state management

#### 2.4 Refactor main.rs (691 lines)
- **Priority**: High
- **Target**: Extract complex logic
- **New structure**:
  - `src/main.rs` - Simplified entry point
  - `src/app.rs` - Application lifecycle and main loop
  - `src/event.rs` - Event handling and routing
  - `src/layout.rs` - Layout calculations and management

### Phase 3: Architecture Improvements (Medium Priority)

#### 3.1 Improve Error Handling
- **Priority**: Medium
- **Action**: Centralize error handling patterns
- **Target**: Create `src/error.rs` for unified error types

#### 3.2 Separate Concerns
- **Priority**: Medium
- **Action**: Better separation between UI and business logic
- **Target**: Move business logic out of UI modules

#### 3.3 Add Configuration Documentation
- **Priority**: Medium
- **Action**: Document all configuration options
- **Target**: Update README.md with comprehensive configuration guide

### Phase 4: Testing and Quality (Low Priority)

#### 4.1 Improve Test Coverage
- **Priority**: Low
- **Action**: Add unit tests for refactored modules
- **Target**: Maintain 100% test pass rate

#### 4.2 Add Integration Tests
- **Priority**: Low
- **Action**: Add integration tests for new module structure
- **Target**: Ensure refactoring doesn't break functionality

#### 4.3 Documentation Updates
- **Priority**: Low
- **Action**: Add inline documentation
- **Target**: Document all public APIs

## Execution Plan

### Task Checklist

#### Phase 1: Dead Code Cleanup
- [x] Remove `#![allow(dead_code)]` from ui.rs
- [x] Remove `#![allow(dead_code)]` from shared_state.rs
- [x] Remove `#![allow(dead_code)]` from monitor.rs
- [x] Remove `#![allow(dead_code)]` from git_worker.rs
- [x] Remove individual `#[allow(dead_code)]` annotations from pane.rs
- [x] Remove individual `#[allow(dead_code)]` annotations from git.rs
- [x] Remove individual `#[allow(dead_code)]` annotations from config.rs
- [x] Remove unused imports identified by clippy
- [x] Audit and remove unused dependencies from Cargo.toml
- [x] Verify all tests pass after cleanup

#### Phase 2: Module Splitting
- [ ] Create `src/pane/mod.rs` and move core functionality
- [ ] Create `src/pane/file_tree.rs` and extract file tree logic
- [ ] Create `src/pane/monitor.rs` and extract monitor logic
- [ ] Create `src/pane/diff.rs` and extract diff logic
- [ ] Create `src/pane/commit_picker.rs` and extract commit picker logic
- [ ] Create `src/pane/advice.rs` and extract advice panel logic
- [ ] Remove original `src/pane.rs` after migration
- [ ] Create `src/ui/mod.rs` and move core UI functionality
- [ ] Create `src/ui/app.rs` and extract application state
- [ ] Create `src/ui/render.rs` and extract rendering logic
- [ ] Create `src/ui/theme.rs` and extract theme management
- [ ] Remove original `src/ui.rs` after migration
- [ ] Create `src/shared_state/mod.rs` and move core functionality
- [ ] Create `src/shared_state/git.rs` and extract git state logic
- [ ] Create `src/shared_state/llm.rs` and extract llm state logic
- [ ] Create `src/app.rs` and extract main application logic
- [ ] Create `src/event.rs` and extract event handling
- [ ] Create `src/layout.rs` and extract layout calculations
- [ ] Simplify `src/main.rs` to just entry point
- [ ] Verify all tests pass after refactoring

#### Phase 3: Architecture Improvements
- [ ] Create `src/error.rs` with unified error types
- [ ] Update all modules to use unified error handling
- [ ] Separate UI and business logic in affected modules
- [ ] Update README.md with comprehensive configuration documentation
- [ ] Verify all tests pass after improvements

#### Phase 4: Testing and Quality
- [ ] Add unit tests for new modules
- [ ] Add integration tests for refactored components
- [ ] Add inline documentation for public APIs
- [ ] Verify all tests pass after quality improvements

## Success Criteria

1. **Zero clippy warnings** without any `#[allow(dead_code)]` annotations
2. **All tests pass** (maintain current 114 passing tests)
3. **Improved modularity** - no module over 1,000 lines
4. **Better separation of concerns** - clear boundaries between UI and business logic
5. **Comprehensive documentation** - all configuration options documented
6. **Maintained functionality** - no loss of existing features

## Risk Management

### High Risk Tasks
- **Module splitting**: May break imports and require extensive refactoring
- **Dead code removal**: May remove code that's actually used indirectly
- **Main.rs refactoring**: May break application startup or event handling

### Mitigation Strategies
1. **Work incrementally**: Test after each small change
2. **Use git frequently**: Commit after each successful task
3. **Run tests often**: Ensure no regressions introduced
4. **Keep backups**: Use git branches for major changes

## Tools and Commands

### Useful Commands
```bash
# Check for dead code
cargo clippy -- -W dead_code

# Check for unused imports
cargo clippy -- -W unused_imports

# Check for unused dependencies
cargo install cargo-machete
cargo machete

# Run tests
cargo test

# Build check
cargo check
```

### Git Workflow
```bash
# Create branch for spring cleaning
git checkout -b spring-cleaning

# Commit after each phase
git commit -m "Phase 1: Remove dead code annotations"

# Tag major milestones
git tag -a spring-cleaning-phase-1 -m "Completed dead code cleanup"
```

## Estimated Timeline

- **Phase 1**: 2-3 hours (dead code cleanup)
- **Phase 2**: 6-8 hours (module splitting)
- **Phase 3**: 2-3 hours (architecture improvements)
- **Phase 4**: 1-2 hours (testing and documentation)

**Total estimated time**: 11-16 hours

## Notes

- This plan should be executed incrementally
- Each task should be verified with `cargo check` and `cargo test`
- Use frequent git commits to track progress and enable rollback
- Focus on maintaining functionality while improving structure
- Prioritize tasks that provide the most maintainability benefit
