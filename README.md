# GRW - Git Repository Watcher

A terminal-based user interface (TUI) for monitoring git repositories in real-time. Built with Rust and Ratatui.

## Features

- **Real-time monitoring**: Automatically detects changes in your git repository
- **File tree view**: Hierarchical display of changed files with directories
- **Diff visualization**: Color-coded git diffs (green for additions, red for deletions)
- **Vim-like keybindings**: Intuitive navigation for vim users
- **Status bar**: Shows repository info, branch, last commit, and change statistics
- **Help system**: Built-in help page with all keybindings

## Keybindings

### Navigation
- `Tab` / `g t` - Next file
- `Shift+Tab` / `g T` - Previous file

### Scrolling
- `j` / `Down` / `Ctrl+e` - Scroll down one line
- `k` / `Up` / `Ctrl+y` - Scroll up one line
- `PageDown` - Scroll down one page
- `PageUp` - Scroll up one page
- `g g` - Go to top of diff
- `Shift+G` - Go to bottom of diff

### Other
- `?` - Show/hide help
- `Esc` - Exit help page
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
docker run -it --rm -v $(pwd):/repo ghcr.io/<username>/grw:latest
```

## Usage

Run the application from any git repository:

```bash
grw
```

The application will:
1. Monitor the current git repository
2. Display changed files in a tree structure on the left
3. Show git diffs in the main panel
4. Update automatically every 500ms

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