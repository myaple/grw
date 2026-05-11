# AGENTS.md

## Project Overview

GRW (Git Repository Watcher) is a terminal-based TUI for monitoring git repositories in real-time. Written in Rust, built with Ratatui and Crossterm.

## Build & Run Commands

- **Build**: `cargo build`
- **Run**: `cargo run`
- **Test**: `cargo test --all`
- **Lint**: `cargo clippy -- -D warnings`
- **Format**: `cargo fmt --check` (verify) / `cargo fmt` (apply)
- **Release build**: `cargo build --release`

Always run `cargo clippy` and `cargo fmt --check` after making changes.

## Project Structure

```
src/
  main.rs              - Entry point, async runtime setup
  lib.rs               - Library root, module declarations
  config.rs            - CLI args (clap) and config file loading
  logging.rs           - Logging setup (env_logger)
  ui.rs                - Main TUI rendering loop and layout
  shared_state.rs      - Central shared state (scc HashMap) coordinator
  git/
    mod.rs             - Git module root
    operations.rs      - Git operations (status, diff, log)
    repository.rs      - Repository abstraction over git2
    types.rs           - Git-related types (FileEntry, DiffLine, etc.)
    worker.rs          - Background git worker (tokio task)
  pane/
    mod.rs             - Pane module root, Pane trait
    keys.rs            - Keybinding definitions
    diff_pane.rs       - Inline diff view
    side_by_side_diff_pane.rs - Side-by-side diff view
    file_tree_pane.rs  - Changed files tree
    help_pane.rs       - Help overlay
    status_bar_pane.rs - Bottom status bar
    monitor_pane.rs    - Monitor command output pane
    commit_picker_pane.rs   - Commit history picker
    commit_summary_pane.rs  - Commit summary display
  monitor.rs           - Monitor command execution worker
  bin/                 - Empty (reserved for future binaries)
tests/
  unit/                - Unit tests (empty, to be populated)
build.rs               - Build script, embeds git SHA at compile time
```

## Architecture

- **Shared state architecture** using lock-free `scc::HashMap` (not channels)
- `SharedStateManager` coordinates `GitSharedState`, `LlmSharedState`, and `MonitorSharedState`
- Background workers (git worker, LLM worker, monitor worker) write directly to shared state
- Main thread reads shared state for UI rendering on a 500ms tick
- Async runtime: Tokio with `#[tokio::main]`

## Code Conventions

- Rust edition 2024
- Error handling: `color_eyre` / `eyre::Result`
- CLI: `clap` with derive macros
- No comments in code unless explicitly requested
- Follow existing patterns in neighboring files when adding new code

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `ratatui` | TUI framework |
| `crossterm` | Terminal backend |
| `git2` | Git operations (libgit2 bindings) |
| `tokio` | Async runtime |
| `clap` | CLI argument parsing |
| `scc` | Lock-free concurrent HashMap |
| `color_eyre` | Error handling |
| `serde` / `serde_json` | Config file serialization |
| `chrono` | Timestamps |
| `textwrap` | Text wrapping in UI |
| `unicode-segmentation` | Unicode-aware string operations |

## CI

GitHub Actions workflow at `.github/workflows/ci.yml`:
1. `cargo build --release --locked`
2. `cargo test --release --locked --all`
3. Docker build and push to GHCR (on push to master only)

## Notes

- The build script (`build.rs`) reads the git HEAD SHA at compile time — builds outside a git repo will show `"unknown"` as the version SHA
- Config file location: `~/.config/grw/config.json`
- Log file location: `~/.local/state/grw/grw.log`
- Default branch is `master`
