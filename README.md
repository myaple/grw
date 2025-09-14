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
- **Light/Dark themes**: Toggle between light and dark themes

## Keybindings

### General
- `?` - Show/hide help
- `Esc` - Exit help page
- `Ctrl+h` - Toggle diff panel visibility
- `Ctrl+o` - Toggle monitor pane visibility
- `Ctrl+t` - Toggle light/dark theme
- `q` / `Ctrl+c` - Quit application

### Pane Modes
- `Ctrl+d` - Switch to inline diff view
- `Ctrl+s` - Switch to side-by-side diff view
- `Ctrl+l` - Switch to LLM advice pane

### File Tree
- `Tab` / `g t` - Next file
- `Shift+Tab` / `g T` - Previous file

### Diff View
- `j` / `Down` / `Ctrl+e` - Scroll down
- `k` / `Up` / `Ctrl+y` - Scroll up
- `PageDown` - Page down
- `PageUp` - Page up
- `g g` - Go to top
- `Shift+G` - Go to bottom

### Monitor
- `Alt+j` / `Alt+Down` - Scroll down
- `Alt+k` / `Alt+Up` - Scroll up

### LLM Advice
- `j` / `k` - Scroll up/down
- `/` - Enter input mode to ask a question
- `Enter` - Submit the question to the LLM
- `Esc` - Exit input mode

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
- `--theme <THEME>` - Set initial theme (light or dark)
- `--llm-provider <PROVIDER>` - LLM provider to use for advice (e.g., openai)
- `--llm-model <MODEL>` - LLM model to use for advice
- `--llm-api-key <KEY>` - API key for the LLM provider
- `--llm-interval <SECONDS>` - Interval in seconds for LLM advice refresh

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
```

### Configuration File

GRW supports a configuration file at `~/.config/grw/config.json` that can be used to persist settings:

```json
{
  "debug": false,
  "no_diff": false,
  "monitor_command": "git status --short",
  "monitor_interval": 5,
  "theme": "dark"
}
```

Or a minimal configuration with only some settings:

```json
{
  "debug": true,
  "theme": "light"
}
```

Configuration options:
- `debug` (boolean): Enable debug logging (optional, default: false)
- `no_diff` (boolean): Hide diff panel, show only file tree (optional, default: false)
- `monitor_command` (string): Command to run in monitor pane (optional)
- `monitor_interval` (number): Interval in seconds for monitor command refresh (optional)
- `theme` (string): Initial theme setting (light or dark) (optional)
- `llm` (object): LLM provider configuration (optional)
  - `provider` (string): LLM provider (e.g., "openai")
  - `model` (string): LLM model name
  - `api_key` (string): API key for the LLM provider
  - `interval` (number): Interval in seconds for LLM advice refresh

A full configuration with LLM settings might look like this:

```json
{
  "debug": false,
  "no_diff": false,
  "monitor_command": "git status --short",
  "monitor_interval": 5,
  "theme": "dark",
  "llm": {
    "provider": "openai",
    "model": "gpt-4",
    "api_key": "your-api-key-here",
    "interval": 120
  }
}
```

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