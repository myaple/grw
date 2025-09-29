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
- **AI-Powered Advice**: Get actionable suggestions for improving your code changes from an integrated AI assistant.

## AI-Powered Advice Panel

GRW includes an AI-powered advice panel that provides actionable suggestions for improving your code changes. It analyzes your diff and offers feedback to enhance code quality, performance, and readability.

- **Activation**: Press `Ctrl+l` to open the advice panel.
- **Interaction**: Use the chat interface to ask questions or request further clarification.
- **Configuration**: Requires an LLM API key and can be configured via command-line arguments or the `config.json` file.

## Keybindings

### General
- `?` - Show/hide help
- `Esc` - Exit help page or active panel (like Advice Panel)
- `Ctrl+h` - Toggle diff panel visibility
- `Ctrl+b` - Toggle changed files pane visibility
- `Ctrl+o` / `Ctrl+m` - Toggle monitor pane visibility
- `Ctrl+l` - Toggle AI advice panel
- `Ctrl+t` - Toggle light/dark theme
- `Ctrl+P` - Enter commit picker mode
- `Ctrl+W` - Return to working directory view
- `q` / `Ctrl+c` - Quit application

### Pane Modes
- `Ctrl+d` - Switch to inline diff view (or exit Advice Panel)
- `Ctrl+s` - Switch to side-by-side diff view

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

### Commit Picker
- `j` / `k` / `↑` / `↓` - Navigate commits
- `g t` - Next commit
- `g T` - Previous commit
- `Enter` - Select commit
- `Esc` - Exit commit picker

### Advice Panel
- `/` - Activate chat input
- `Enter` - Send message (when input is active)
- `Esc` - Deactivate chat input
- `?` - Show help

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
- `--hide-changed-files-pane` - Hide changed files pane, show only diff
- `--monitor-command <COMMAND>` - Command to run in monitor pane
- `--monitor-interval <SECONDS>` - Interval in seconds for monitor command refresh
- `--theme <THEME>` - Set initial theme (light or dark)
- `--llm-provider <PROVIDER>` - LLM provider to use for AI features (e.g., openai)
- `--llm-model <MODEL>` - Default LLM model for all AI features
- `--llm-summary-model <MODEL>` - Specific model for commit summaries
- `--llm-advice-model <MODEL>` - Specific model for generating advice
- `--llm-api-key <KEY>` - API key for the LLM provider
- `--llm-base-url <URL>` - Base URL for the LLM provider
- `--commit-history-limit <NUMBER>` - Maximum number of commits to load (default: 100)
- `--commit-cache-size <NUMBER>` - Maximum number of commits to cache (default: 200)
- `--summary-preload-enabled <BOOL>` - Enable summary pre-loading (default: true)
- `--summary-preload-count <NUMBER>` - Number of summaries to pre-load (default: 5)

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
- `hide_changed_files_pane` (boolean): Hide changed files pane, show only diff (optional, default: false)
- `monitor_command` (string): Command to run in monitor pane (optional)
- `monitor_interval` (number): Interval in seconds for monitor command refresh (optional)
- `theme` (string): Initial theme setting (light or dark) (optional)
- `commit_history_limit` (number): Maximum number of commits to load in commit picker (optional, default: 100)
- `commit_cache_size` (number): Maximum number of commits to cache in shared state (optional, default: 200)
- `summary_preload_enabled` (boolean): Enable automatic summary preloading (optional, default: true)
- `summary_preload_count` (number): Number of summaries to preload ahead (optional, default: 5)
- `llm` (object): LLM provider configuration (optional)
  - `provider` (string): LLM provider (e.g., "openai")
  - `model` (string): Default LLM model name
  - `summary_model` (string): Specific model for commit summaries (optional)
  - `advice_model` (string): Specific model for generating advice (optional)
  - `api_key` (string): API key for the LLM provider
  - `base_url` (string): Base URL for the LLM provider
- `advice` (object): Configuration for the AI advice panel (optional)
  - `enabled` (boolean): Enable the advice panel feature (optional, default: true)
  - `max_improvements` (number): Number of improvement suggestions to request (optional, default: 3)
  - `chat_history_limit` (number): Max number of messages in chat history (optional, default: 10)
  - `timeout_seconds` (number): Timeout for AI requests in seconds (optional, default: 60)
  - `context_lines` (number): Lines of context to include around changes (optional, default: 3)

A full configuration with LLM settings and shared state tuning might look like this:

```json
{
  "debug": false,
  "no_diff": false,
  "hide_changed_files_pane": false,
  "monitor_command": "git status --short",
  "monitor_interval": 5,
  "theme": "dark",
  "commit_history_limit": 150,
  "commit_cache_size": 300,
  "summary_preload_enabled": true,
  "summary_preload_count": 8,
  "llm": {
    "provider": "openai",
    "model": "gpt-4o-mini",
    "summary_model": "gpt-4o-mini",
    "advice_model": "gpt-4-turbo",
    "api_key": "your-api-key-here",
    "base_url": "https://api.openai.com/v1"
  },
  "advice": {
    "enabled": true,
    "max_improvements": 5,
    "chat_history_limit": 20,
    "timeout_seconds": 90,
    "context_lines": 5
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

## Architecture

GRW uses a modern shared state architecture built on lock-free concurrent data structures from the `scc` crate. This design provides better performance and lower latency compared to traditional channel-based communication.

### Shared State Components

- **GitSharedState**: Manages repository data, commit cache, and file diffs using concurrent HashMap structures
- **LlmSharedState**: Handles LLM summary and advice caching with active task tracking
- **MonitorSharedState**: Stores monitor command output and timing information
- **SharedStateManager**: Central coordinator for all shared state components

### Key Benefits

- **Lock-free operations**: All data structures use atomic operations for thread-safe access
- **Better cache locality**: Shared memory reduces overhead compared to message passing
- **Direct access**: Main thread can read data directly without waiting for channel messages
- **Concurrent caching**: Multiple workers can update caches simultaneously without blocking

### Performance Characteristics

- Real-time git status updates with minimal overhead
- Efficient LLM summary caching and preloading
- Responsive UI updates through direct shared state access
- Automatic cleanup of stale tasks and cached data

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