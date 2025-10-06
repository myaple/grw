# Claude LLM Instructions

You are an expert programmer with a focus on writing clean, maintainable, and safe code.
Please follow these instructions when reviewing code.

## Critical Implementation Notes

### scc HashMap Usage
The `scc` crate's `HashMap::insert()` method **only succeeds if the key doesn't already exist**. Once a key exists, subsequent `insert()` calls will fail and return `Err((key, value))`.

**Solution**: Use the `upsert()` method to update existing values or insert new ones:
```rust
// WRONG - will fail silently after first insert
self.map.insert(key, value);

// CORRECT - use upsert to update or insert
self.map.upsert(key, value);
```

**Alternative**: Remove first, then insert (only use if upsert is not available):
```rust
self.map.remove(&key);
self.map.insert(key, value);
```

This pattern is used in `shared_state.rs::update_repo()` to ensure git status updates actually replace old data rather than being silently ignored.

### git2 Path Handling - CRITICAL
The `git2` crate has very strict path requirements that can cause subtle bugs:

**Problem**: `git2` functions require **relative paths from the repository root**, but many parts of the codebase work with **absolute paths**.

**What breaks**: When absolute paths are passed to git2 functions:
- `get_working_tree_diff()` returns 0 lines, +0 -0 for any file
- `get_commit_file_diff()` returns empty results
- `diff_index_to_workdir()` produces no deltas
- All git operations appear to "not work"

**Solution**: Always convert absolute paths to relative paths before calling git2:

```rust
// Convert absolute path to relative path for git2 operations
let relative_path = match absolute_path.strip_prefix(&repo_root_path) {
    Ok(rel_path) => rel_path,
    Err(_) => {
        log::debug!("Failed to convert absolute path to relative: {:?} (repo: {:?})", absolute_path, repo_root_path);
        absolute_path  // fallback, but this indicates a bug
    }
};

// Now use relative_path with git2
git_operations::get_working_tree_diff(&repo, relative_path)?;
```

**Key implementations**:
- `src/git_worker.rs::generate_diff()` - converts absolute paths to relative paths for working directory diffs
- `src/ui.rs::get_commit_diff_content()` - ensures repo path is absolute for proper path stripping for commit diffs

**Debugging tip**: If git operations return empty results when they shouldn't, check if you're passing absolute paths to git2 functions. Use debug logging to verify the exact paths being passed.

### Theme Color Usage - IMPORTANT
The application supports both light and dark themes through the `Theme` enum in `src/ui.rs`. **Always use theme colors instead of hardcoded colors** to ensure the UI works properly in both light and dark modes.

**Problem**: Hardcoded colors like `Color::Green`, `Color::Cyan`, etc. will look good in only one theme mode and may be unreadable in the other.

**Available theme methods**:
```rust
// Get theme from App instance
let theme = app.get_theme();

// Basic colors
theme.background_color()   // Black (dark) / White (light)
theme.foreground_color()   // White (dark) / Black (light)

// Semantic colors
theme.primary_color()      // Cyan (dark) / Blue (light)
theme.secondary_color()    // Yellow (both modes)
theme.error_color()        // Red (dark) / LightRed (light)
theme.highlight_color()    // Blue (dark) / LightBlue (light)
theme.border_color()       // Gray (dark) / DarkGray (light)

// Specialized colors
theme.directory_color()    // Cyan (dark) / Blue (light)
theme.added_color()        // Green (both modes)
theme.removed_color()      // Red (dark) / LightRed (light)
theme.unchanged_color()    // Same as foreground_color()
```

**Solution**: Always access theme through `app.get_theme()` in render methods and use theme-appropriate colors:

```rust
// WRONG - hardcoded colors
Line::from("Hello").fg(Color::Green)
Style::default().fg(Color::Cyan)

// CORRECT - use theme colors
let theme = app.get_theme();
Line::from("Hello").fg(theme.primary_color())
Style::default().fg(theme.foreground_color())
```

**Theme Toggle**: Ctrl+T automatically toggles between light and dark themes via `app.toggle_theme()`. All UI components should respect this without additional changes.

**Key implementations**:
- `src/pane/advice_panel.rs` - Updated all chat colors to use theme colors
- `src/ui.rs` - Contains the `Theme` enum with color definitions for both modes
