# GRW - Git Repository Watcher

A terminal-based user interface (TUI) for monitoring git repositories in real-time. Built with Rust and Ratatui.

## Features

- **Real-time monitoring**: Automatically detects changes in your git repository every 500ms
- **File tree view**: Hierarchical display of changed files with directories
- **Diff visualization**: Color-coded git diffs (green for additions, red for deletions)
- **Dual diff modes**: Single-pane and side-by-side diff views
- **Panel toggling**: Hide/show diff panel for focused file tree view
- **Vim-like keybindings**: Intuitive navigation for vim users
- **Status bar**: Shows repository info, branch, last commit, and change statistics with automatic text wrapping
- **Help system**: Built-in help page with all keybindings
- **Logging**: Comprehensive logging with debug mode for troubleshooting
- **Responsive UI**: Adapts to terminal size with intelligent header wrapping
- **Light/Dark themes**: Toggle between light and dark themes with automatic adaptation

## Keybindings

### File Navigation
- `Tab` / `g t` - Next file
- `Shift+Tab` / `g T` - Previous file

### Scrolling
- `j` / `Down` / `Ctrl+e` - Scroll down one line
- `k` / `Up` / `Ctrl+y` - Scroll up one line
- `PageDown` - Scroll down one page
- `PageUp` - Scroll up one page
- `g g` - Go to top of diff
- `Shift+G` - Go to bottom of diff

### Diff View Modes
- `Ctrl+S` - Switch to side-by-side diff view
- `Ctrl+D` - Switch to single-pane diff view

### Panel Controls
- `Ctrl+H` - Toggle diff panel visibility
- `Ctrl+O` - Toggle monitor pane visibility

### Theme Controls
- `Ctrl+T` - Toggle between light and dark themes

### Help and Interface
- `?` - Show/hide help
- `Esc` - Exit help page

### Application Control
- `q` / `Ctrl+C` - Quit application

## Installation

### From Source

```bash
git clone <repository-url>
cd grw
cargo build --release
```

### Docker

```bash
docker run -it --rm -v $(pwd):/repo ghcr.io/<your-github-username>/grw:latest
```

## Usage

Run the application from any git repository:

```bash
grw
```

### Command Line Options

- `-v, --version` - Print version information and exit
- `-h, --help` - Print help information
- `-d, --debug` - Enable debug logging
- `--no-diff` - Hide diff panel, show only file tree
- `--monitor-command <COMMAND>` - Command to run in monitor pane
- `--monitor-interval <SECONDS>` - Interval in seconds for monitor command refresh
- `--theme <THEME>` - Set initial theme (light, dark, or auto)

### Examples

```bash
# Normal mode with diff panel
grw

# Hide diff panel for focused file tree view
grw --no-diff

# Enable debug logging for troubleshooting
grw --debug

# Hide diff panel with debug logging
grw --no-diff --debug

# Run a monitor command every 5 seconds
grw --monitor-command "git status --short" --monitor-interval 5

# Run a custom script in monitor pane
grw --monitor-command "./scripts/check-deps.sh" --monitor-interval 10

# Start with light theme
grw --theme light

# Start with dark theme
grw --theme dark

# Use auto theme (detects terminal theme preference)
grw --theme auto
```

### Configuration File

GRW supports a configuration file at `~/.config/grw/config.json` that can be used to persist settings:

```json
{
  "debug": false,
  "no_diff": false,
  "monitor_command": "git status --short",
  "monitor_interval": 5
}
```

Configuration options:
- `debug` (boolean): Enable debug logging (default: false)
- `no_diff` (boolean): Hide diff panel, show only file tree (default: false)  
- `monitor_command` (string): Command to run in monitor pane (optional)
- `monitor_interval` (number): Interval in seconds for monitor command refresh (optional)
- `theme` (string): Initial theme setting (light, dark, or auto) (optional)

Command line arguments override configuration file settings.

### Interface Layout

The application will:
1. Monitor the current git repository
2. Display changed files in a tree structure on the left
3. Show git diffs in the right panel (when visible)
4. Display repository information in the status bar (with automatic text wrapping)
5. Update automatically every 500ms

### Panel Modes

- **Default mode**: Shows both file tree (30%) and diff panel (70%)
- **No-diff mode**: Shows only file tree (100% width)
- **Help mode**: Shows help documentation in place of diff panel or full content area

### Theme System

GRW includes a flexible theme system that supports light and dark modes:

- **Dark theme**: Default theme optimized for terminal use with dark backgrounds
- **Light theme**: Bright theme suitable for light terminal backgrounds or better readability in bright environments
- **Auto theme**: Automatically detects terminal theme preference (when supported)
- **Hotkey toggle**: Use `Ctrl+T` to quickly switch between light and dark themes during runtime
- **Persistent setting**: Theme preference can be saved in the configuration file or set via command line

The theme system intelligently adapts colors for optimal readability in both light and dark modes, ensuring that git diff colors (green for additions, red for deletions) remain clearly visible regardless of the selected theme.

### Logging

The application includes comprehensive logging for troubleshooting and monitoring:

- **Log location**: `~/.local/state/grw/grw.log` (follows XDG Base Directory specification)
- **Default level**: INFO (normal operation information)
- **Debug level**: DEBUG (detailed performance and operation logs)

```bash
# Enable debug logging
grw --debug

# View log file
cat ~/.local/state/grw/grw.log

# Follow log file in real-time
tail -f ~/.local/state/grw/grw.log
```

Debug logs include:
- Performance metrics for git operations
- Render timing information
- File change detection details
- UI state changes

## Development

### Prerequisites

- Rust 1.70+
- Git

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Formatting

```bash
cargo fmt
```

### Linting

```bash
cargo clippy
```

## Docker

The project includes a Dockerfile for building a containerized version:

```bash
docker build -t grw .
```

## License

This project is open source and available under the [MIT License](LICENSE).