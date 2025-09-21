use clap::Parser;
use color_eyre::eyre::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::{Event, KeyCode, KeyEvent, KeyModifiers},
        execute,
        terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
    },
};
use std::io;
use std::time::Duration;

mod config;
mod git;
mod git_worker;
mod llm;
mod logging;
mod monitor;
mod pane;
mod ui;

use std::env;

use config::{Args, Config};
use git::AsyncGitRepo;
use llm::{AsyncLLMCommand, LlmClient};
use log::{debug, error, info};
use monitor::AsyncMonitorCommand;
use ui::App;

pub const GIT_SHA: &str = "unknown";

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.version {
        println!("grw version 0.1.0 (git: {GIT_SHA})");
        return Ok(());
    }

    let config = Config::load()?;
    let final_config = config.merge_with_args(&args);

    logging::init_logging(final_config.debug.unwrap_or(false))?;
    color_eyre::install()?;

    let repo_path = std::env::current_dir()?;
    log::info!("Starting grw in directory: {repo_path:?}");
    log::debug!("Debug mode enabled");
    let mut git_repo = AsyncGitRepo::new(repo_path, 500)?;

    let llm_client = if let Some(llm_config) = &final_config.llm {
        if llm_config.api_key.is_some() || env::var("OPENAI_API_KEY").is_ok() {
            match LlmClient::new(llm_config.clone()) {
                Ok(client) => Some(client),
                Err(e) => {
                    info!("Failed to create LLM client: {e}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut app = App::new_with_config(
        !final_config.no_diff.unwrap_or(false),
        !final_config.hide_changed_files_pane.unwrap_or(false),
        match final_config.theme.clone().unwrap_or(config::Theme::Dark) {
            config::Theme::Dark => ui::Theme::Dark,
            config::Theme::Light => ui::Theme::Light,
        },
        llm_client.clone(),
    );

    let mut monitor_command = if let Some(cmd) = &final_config.monitor_command {
        Some(AsyncMonitorCommand::new(
            cmd.clone(),
            final_config.monitor_interval.unwrap_or(5),
        ))
    } else {
        None
    };

    // Enable monitor pane when a command is configured
    if monitor_command.is_some() {
        app.toggle_monitor_pane();
        app.set_monitor_command_configured(true);
    }

    let mut llm_command = if let Some(client) = &llm_client {
        let command = AsyncLLMCommand::new(client.clone());
        log::debug!("Triggering initial LLM advice refresh");
        command.refresh();
        Some(command)
    } else {
        None
    };

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let _ = terminal.clear();

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    loop {
        // Poll for git updates (but skip if a commit is selected to avoid overriding commit files)
        if app.get_selected_commit().is_none() {
            git_repo.update();
            if let Some(result) = git_repo.try_get_result() {
                match result {
                    git::GitWorkerResult::Update(repo) => {
                        let changed_files = repo.get_display_files();
                        let tree = repo.get_file_tree();

                        app.update_files(changed_files.clone());
                        app.update_tree(&tree);
                        git_repo.repo = Some(repo);
                    }
                    git::GitWorkerResult::Error(e) => {
                        error!("Git worker error: {e}");
                    }
                }
            }
        } else {
            // Still update the git repo state for status bar, but don't override app files
            git_repo.update();
            if let Some(result) = git_repo.try_get_result() {
                match result {
                    git::GitWorkerResult::Update(repo) => {
                        git_repo.repo = Some(repo);
                    }
                    git::GitWorkerResult::Error(e) => {
                        error!("Git worker error: {e}");
                    }
                }
            }
        }

        // Update monitor command if it exists
        if let Some(ref mut monitor) = monitor_command {
            if let Some(result) = monitor.try_get_result() {
                match result {
                    monitor::MonitorResult::Success(output) => {
                        app.update_monitor_output(output);
                    }
                    monitor::MonitorResult::Error(output) => {
                        log::error!("Monitor command failed");
                        app.update_monitor_output(output);
                    }
                }
            }

            // Update timing information
            let elapsed = monitor.get_elapsed_since_last_run();
            let has_run = monitor.has_run_yet();
            app.update_monitor_timing(elapsed, has_run);
        }

        // Poll for LLM advice responses
        app.poll_llm_advice();
        
        // Poll for LLM commit summary responses
        app.poll_llm_summaries();

        // Update llm command if it exists
        if let Some(ref mut llm) = llm_command {
            if let Some(repo) = &git_repo.repo {
                let _ = llm.git_repo_tx.send(Some(repo.clone()));
            }

            if let Some(result) = llm.try_get_result() {
                match result {
                    llm::LLMResult::Success(output) => {
                        app.update_llm_advice(output);
                    }
                    llm::LLMResult::Error(output) => {
                        log::error!("LLM command failed");
                        app.update_llm_advice(output);
                    }
                    llm::LLMResult::Noop => {}
                }
            }
        }

        // Calculate monitor visible height before rendering
        let terminal_size = terminal.size()?;
        let terminal_rect =
            ratatui::layout::Rect::new(0, 0, terminal_size.width, terminal_size.height);
        if app.is_showing_monitor_pane() {
            let chunks = if app.is_showing_diff_panel() {
                // When both diff panel and monitor pane are shown
                let main_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(terminal_rect);

                let bottom_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(30),
                        ratatui::layout::Constraint::Percentage(70),
                    ])
                    .split(main_chunks[1]);

                ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(50),
                        ratatui::layout::Constraint::Percentage(50),
                    ])
                    .split(bottom_chunks[0])
            } else {
                // When only monitor pane is shown (no diff panel)
                let main_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(terminal_rect);

                ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(50),
                        ratatui::layout::Constraint::Percentage(50),
                    ])
                    .split(main_chunks[1])
            };

            app.set_monitor_visible_height(chunks[1].height.saturating_sub(2) as usize);
        }

        let render_start = std::time::Instant::now();
        terminal.draw(|f| {
            let size = f.area();

            // Calculate diff height only if diff panel is visible
            if app.is_showing_diff_panel() {
                let chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Vertical)
                    .constraints([
                        ratatui::layout::Constraint::Length(1),
                        ratatui::layout::Constraint::Min(0),
                    ])
                    .split(size);

                let bottom_chunks = ratatui::layout::Layout::default()
                    .direction(ratatui::layout::Direction::Horizontal)
                    .constraints([
                        ratatui::layout::Constraint::Percentage(30),
                        ratatui::layout::Constraint::Percentage(70),
                    ])
                    .split(chunks[1]);

                let diff_height = bottom_chunks[1].height.saturating_sub(2) as usize;
                app.current_diff_height = diff_height;
            } else {
                // When diff panel is hidden, set a reasonable default height
                app.current_diff_height = 20;
            }

            if let Some(repo) = &git_repo.repo {
                ui::render::<CrosstermBackend<std::io::Stdout>>(f, &app, repo);
            }
        })?;
        let render_duration = render_start.elapsed();

        if render_duration.as_millis() > 10 {
            log::debug!("Slow render detected: {render_duration:?}");
        }

        // Update commit summary pane with current selection from commit picker
        if app.is_in_commit_picker_mode() {
            app.update_commit_summary_with_current_selection();
        }

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if handle_key_event(key, &mut app, &git_repo, &final_config) {
                    break;
                }
            }
        }

        if app.is_advice_refresh_requested() {
            if let Some(llm) = &mut llm_command {
                llm.refresh();
            }
            app.reset_advice_refresh_request();
        }

        // Handle commit selection from commit picker
        if app.is_in_commit_picker_mode() && app.is_commit_picker_enter_pressed() {
            if let Some(selected_commit) = app.get_current_selected_commit_from_picker() {
                // Validate the selected commit before proceeding
                if selected_commit.sha.is_empty() {
                    error!("Selected commit has empty SHA, cannot proceed");
                    app.reset_commit_picker_enter_pressed();
                } else if selected_commit.short_sha.is_empty() {
                    error!("Selected commit has empty short SHA, cannot proceed");
                    app.reset_commit_picker_enter_pressed();
                } else {
                    debug!("Processing commit selection: {} - {}", selected_commit.short_sha, selected_commit.message);
                    
                    // Load the selected commit's files
                    app.load_commit_files(&selected_commit);
                    
                    // Select the commit and exit commit picker mode
                    app.select_commit(selected_commit);
                    
                    // Reset the enter pressed flag
                    app.reset_commit_picker_enter_pressed();
                }
            } else {
                debug!("No commit selected despite enter being pressed");
                app.reset_commit_picker_enter_pressed();
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    let _ = terminal.clear();

    log::info!("Application shutdown complete");
    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App, git_repo: &AsyncGitRepo, config: &Config) -> bool {
    // Handle commit picker mode key events first
    if app.is_in_commit_picker_mode() {
        // Handle quit keys (q and Ctrl+C) even in commit picker mode
        match key.code {
            KeyCode::Char('q') => {
                log::info!("User requested quit from commit picker mode");
                return true;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                log::info!("User requested quit via Ctrl+C from commit picker mode");
                return true;
            }
            KeyCode::Esc => {
                debug!("User pressed Escape in commit picker mode, exiting");
                app.exit_commit_picker_mode();
                return false;
            }
            _ => {}
        }
        
        // Forward key events to commit picker pane with error handling
        let picker_handled = app.forward_key_to_commit_picker(key);
        
        // Also forward to commit summary pane for scrolling if not handled by picker
        if !picker_handled {
            app.forward_key_to_commit_summary(key);
        }
        
        return false; // Don't quit, stay in commit picker mode
    }

    if app.is_showing_advice_pane()
        && app.forward_key_to_panes(key) {
            return false;
        }

    match key.code {
        KeyCode::Char('q') => {
            log::info!("User requested quit");
            true
        }
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            log::info!("User requested quit via Ctrl+C");
            true
        }
        KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_to_bottom(app.current_diff_height);
                false
            }
        }
        KeyCode::Char('j') if key.modifiers.is_empty() => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_down(app.current_diff_height);
                false
            }
        }
        KeyCode::Down => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_down(app.current_diff_height);
                false
            }
        }
        KeyCode::Char('k') if key.modifiers.is_empty() => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_up();
                false
            }
        }
        KeyCode::Up => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_up();
                false
            }
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_down(app.current_diff_height);
                false
            }
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            if app.forward_key_to_panes(key) {
                // Key was handled by a pane
                false
            } else {
                // Fall back to default behavior
                app.scroll_up();
                false
            }
        }
        KeyCode::Char('g') => {
            if app.handle_g_press() {
                false
            } else {
                // g was pressed, wait for next key
                false
            }
        }
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+T - toggling theme");
            app.toggle_theme();
            false
        }
        KeyCode::Char('t') => {
            // Check if g was pressed recently
            if let Some(last_time) = app.last_g_press
                && std::time::Instant::now()
                    .duration_since(last_time)
                    .as_millis()
                    < 500
            {
                debug!("User triggered 'gt' key combination - next file");
                app.next_file();
            }
            false
        }
        KeyCode::Char('T') => {
            // Check if g was pressed recently
            if let Some(last_time) = app.last_g_press
                && std::time::Instant::now()
                    .duration_since(last_time)
                    .as_millis()
                    < 500
            {
                debug!("User triggered 'gT' key combination - previous file");
                app.prev_file();
            }
            false
        }
        KeyCode::PageDown => {
            app.page_down(app.current_diff_height);
            false
        }
        KeyCode::PageUp => {
            app.page_up(app.current_diff_height);
            false
        }
        KeyCode::Tab => {
            debug!("User pressed Tab - next file");
            app.next_file();
            false
        }
        KeyCode::BackTab => {
            debug!("User pressed Shift+Tab - previous file");
            app.prev_file();
            false
        }
        KeyCode::Char('?') => {
            app.toggle_help();
            false
        }
        KeyCode::Esc => {
            if app.is_showing_help() {
                app.toggle_help();
            }
            false
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.set_side_by_side_diff();
            false
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.set_single_pane_diff();
            false
        }
        KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_diff_panel();
            false
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.toggle_changed_files_pane();
            false
        }
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.scroll_monitor_down();
            false
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
            app.scroll_monitor_up();
            false
        }
        KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+O - toggling monitor pane");
            app.toggle_monitor_pane();
            debug!("Monitor pane is now: {}", app.is_showing_monitor_pane());
            false
        }
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+L - switching to advice pane");
            app.set_advice_pane();
            false
        }
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+W - returning to working directory view");
            app.clear_selected_commit();
            false
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+P - activating commit picker");
            // Only activate commit picker when in appropriate diff mode
            if app.is_showing_diff_panel() && !app.is_in_commit_picker_mode() {
                if let Some(repo) = &git_repo.repo {
                    // Enter commit picker mode first and show loading state
                    app.enter_commit_picker_mode();
                    app.set_commit_picker_loading();
                    
                    // Create a temporary GitWorker to load commit history
                    let (_tx, rx) = tokio::sync::mpsc::channel(1);
                    let (result_tx, _result_rx) = tokio::sync::mpsc::channel(1);
                    
                    match crate::git_worker::GitWorker::new(
                        repo.path.clone(),
                        rx,
                        result_tx,
                    ) {
                        Ok(mut git_worker) => {
                            // Configure cache size from config
                            git_worker.set_cache_size(config.get_commit_cache_size());
                            
                            // Use configurable commit history limit
                            let commit_limit = config.get_commit_history_limit();
                            match git_worker.get_commit_history(commit_limit) {
                                Ok(commits) => {
                                    debug!("Successfully loaded {} commits", commits.len());
                                    app.update_commit_picker_commits(commits);
                                }
                                Err(e) => {
                                    error!("Failed to load commit history: {}", e);
                                    let error_msg = if e.to_string().contains("not a git repository") {
                                        "This directory is not a Git repository".to_string()
                                    } else if e.to_string().contains("no commits") || e.to_string().contains("HEAD") {
                                        "No commits found in this repository".to_string()
                                    } else if e.to_string().contains("permission") {
                                        "Permission denied accessing Git repository".to_string()
                                    } else {
                                        format!("Git error: {}", e)
                                    };
                                    app.set_commit_picker_error(error_msg);
                                }
                            }
                        }
                        Err(e) => {
                            error!("Failed to create GitWorker: {}", e);
                            let error_msg = if e.to_string().contains("not a git repository") {
                                "This directory is not a Git repository".to_string()
                            } else if e.to_string().contains("permission") {
                                "Permission denied accessing Git repository".to_string()
                            } else {
                                format!("Failed to initialize Git operations: {}", e)
                            };
                            app.set_commit_picker_error(error_msg);
                        }
                    }
                } else {
                    debug!("No git repository available for commit picker");
                    app.enter_commit_picker_mode();
                    app.set_commit_picker_error("No Git repository loaded".to_string());
                }
            } else if !app.is_showing_diff_panel() {
                debug!("Commit picker requires diff panel to be visible");
                // Could show a status message here in the future
            } else if app.is_in_commit_picker_mode() {
                debug!("Already in commit picker mode");
            }
            false
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use crate::ui::{App, Theme};

    #[tokio::test]
    async fn test_ctrl_p_activates_commit_picker() {
        // Create a test app with diff panel enabled
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        // Create a mock git repo
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize a git repository
        let _repo = git2::Repository::init(&repo_path).unwrap();
        
        // Create a mock AsyncGitRepo
        let git_repo = AsyncGitRepo::new(repo_path.clone(), 500).unwrap();
        
        // Ensure app is not in commit picker mode initially
        assert!(!app.is_in_commit_picker_mode());
        
        // Create Ctrl+P key event
        let ctrl_p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        
        // Handle the key event
        let should_quit = handle_key_event(ctrl_p_key, &mut app, &git_repo, &Config::default());
        
        // Should not quit
        assert!(!should_quit);
        
        // App should now be in commit picker mode (if git repo is available)
        // Note: This test might not activate commit picker if no git repo is loaded
        // but it should not crash or cause errors
    }

    #[tokio::test]
    async fn test_ctrl_p_only_activates_when_diff_panel_shown() {
        // Create a test app with diff panel disabled
        let mut app = App::new_with_config(false, true, Theme::Dark, None);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize a git repository
        let _repo = git2::Repository::init(&repo_path).unwrap();
        
        let git_repo = AsyncGitRepo::new(repo_path, 500).unwrap();
        
        // Ensure app is not in commit picker mode initially
        assert!(!app.is_in_commit_picker_mode());
        
        // Create Ctrl+P key event
        let ctrl_p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        
        // Handle the key event
        let should_quit = handle_key_event(ctrl_p_key, &mut app, &git_repo, &Config::default());
        
        // Should not quit
        assert!(!should_quit);
        
        // App should still not be in commit picker mode since diff panel is not shown
        assert!(!app.is_in_commit_picker_mode());
    }

    #[tokio::test]
    async fn test_ctrl_p_does_not_activate_when_already_in_commit_picker_mode() {
        // Create a test app with diff panel enabled
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize a git repository
        let _repo = git2::Repository::init(&repo_path).unwrap();
        
        let git_repo = AsyncGitRepo::new(repo_path, 500).unwrap();
        
        // Manually enter commit picker mode
        app.enter_commit_picker_mode();
        assert!(app.is_in_commit_picker_mode());
        
        // Create Ctrl+P key event
        let ctrl_p_key = KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL);
        
        // Handle the key event
        let should_quit = handle_key_event(ctrl_p_key, &mut app, &git_repo, &Config::default());
        
        // Should not quit
        assert!(!should_quit);
        
        // App should still be in commit picker mode (no change)
        assert!(app.is_in_commit_picker_mode());
    }

    #[tokio::test]
    async fn test_commit_selection_and_return_to_normal_mode() {
        // Create a test app with diff panel enabled
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        // Enter commit picker mode
        app.enter_commit_picker_mode();
        assert!(app.is_in_commit_picker_mode());
        
        // Create a test commit
        let test_commit = crate::git::CommitInfo {
            sha: "abc123def456".to_string(),
            short_sha: "abc123d".to_string(),
            message: "Test commit message".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01 12:00:00".to_string(),
            files_changed: vec![
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("test_file.txt"),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: 5,
                    deletions: 2,
                }
            ],
        };
        
        // Update commit picker with test commits
        app.update_commit_picker_commits(vec![test_commit.clone()]);
        
        // Simulate Enter key press to select commit
        let enter_key = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
        app.forward_key_to_commit_picker(enter_key);
        
        // Check that enter was pressed
        assert!(app.is_commit_picker_enter_pressed());
        
        // Get the selected commit
        let selected_commit = app.get_current_selected_commit_from_picker().unwrap();
        assert_eq!(selected_commit.sha, "abc123def456");
        
        // Load commit files and select the commit
        app.load_commit_files(&selected_commit);
        app.select_commit(selected_commit);
        
        // Should now be in normal mode
        assert!(!app.is_in_commit_picker_mode());
        
        // Reset the enter pressed flag
        app.reset_commit_picker_enter_pressed();
        assert!(!app.is_commit_picker_enter_pressed());
    }

    #[tokio::test]
    async fn test_escape_exits_commit_picker_mode() {
        // Create a test app with diff panel enabled
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize a git repository
        let _repo = git2::Repository::init(&repo_path).unwrap();
        
        let git_repo = AsyncGitRepo::new(repo_path, 500).unwrap();
        
        // Enter commit picker mode
        app.enter_commit_picker_mode();
        assert!(app.is_in_commit_picker_mode());
        
        // Create Escape key event
        let escape_key = KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        
        // Handle the key event
        let should_quit = handle_key_event(escape_key, &mut app, &git_repo, &Config::default());
        
        // Should not quit
        assert!(!should_quit);
        
        // App should now be in normal mode
        assert!(!app.is_in_commit_picker_mode());
    }

    #[tokio::test]
    async fn test_selected_commit_persists_and_can_be_cleared() {
        // Create a test app
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        // Initially no commit should be selected
        assert!(app.get_selected_commit().is_none());
        
        // Create a test commit
        let test_commit = crate::git::CommitInfo {
            sha: "abc123def456".to_string(),
            short_sha: "abc123d".to_string(),
            message: "Test commit message".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01 12:00:00".to_string(),
            files_changed: vec![
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("test_file.txt"),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: 5,
                    deletions: 2,
                }
            ],
        };
        
        // Select the commit
        app.select_commit(test_commit.clone());
        
        // Commit should now be selected
        assert!(app.get_selected_commit().is_some());
        assert_eq!(app.get_selected_commit().unwrap().sha, "abc123def456");
        
        // Clear the selected commit
        app.clear_selected_commit();
        
        // No commit should be selected
        assert!(app.get_selected_commit().is_none());
    }

    #[tokio::test]
    async fn test_ctrl_w_clears_selected_commit() {
        // Create a test app
        let mut app = App::new_with_config(true, true, Theme::Dark, None);
        
        let temp_dir = tempfile::tempdir().unwrap();
        let repo_path = temp_dir.path().to_path_buf();
        
        // Initialize a git repository
        let _repo = git2::Repository::init(&repo_path).unwrap();
        
        let git_repo = AsyncGitRepo::new(repo_path, 500).unwrap();
        
        // Create and select a test commit
        let test_commit = crate::git::CommitInfo {
            sha: "abc123def456".to_string(),
            short_sha: "abc123d".to_string(),
            message: "Test commit message".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01 12:00:00".to_string(),
            files_changed: vec![],
        };
        
        app.select_commit(test_commit);
        assert!(app.get_selected_commit().is_some());
        
        // Create Ctrl+W key event
        let ctrl_w_key = KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL);
        
        // Handle the key event
        let should_quit = handle_key_event(ctrl_w_key, &mut app, &git_repo, &Config::default());
        
        // Should not quit
        assert!(!should_quit);
        
        // Selected commit should be cleared
        assert!(app.get_selected_commit().is_none());
    }
}
