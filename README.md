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

- `--no-diff` - Hide diff panel, show only file tree
- `-d, --debug` - Enable debug logging
- `-v, --version` - Print version information and exit
- `-h, --help` - Print help information

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
```

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