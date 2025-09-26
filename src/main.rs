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
mod shared_state;
mod ui;

use std::env;
use std::sync::Arc;

use config::{Args, Config};
use llm::LlmClient;
use log::{debug, error, info};
use monitor::AsyncMonitorCommand;
use shared_state::SharedStateManager;
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

    // Initialize shared state manager
    let shared_state_manager = SharedStateManager::new();
    let shared_state_config = config.get_shared_state_config();
    if let Err(e) = shared_state_manager.initialize(Some(&shared_state_config)) {
        error!(
            "Failed to initialize shared state with configuration: {}",
            e
        );
        return Err(color_eyre::eyre::eyre!(
            "Failed to initialize shared state: {}",
            e
        ));
    }
    info!("Shared state manager initialized successfully");

    // Create GitWorker with shared state and start it running continuously
    let mut git_worker = crate::git_worker::GitWorker::new(
        repo_path.clone(),
        Arc::clone(&shared_state_manager.git_state()),
    )?;

    // Start the GitWorker in a background task
    tokio::spawn(async move {
        if let Err(e) = git_worker.run_continuous(500).await {
            error!("GitWorker continuous run failed: {}", e);
        }
    });

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
        Arc::clone(&shared_state_manager.llm_state()),
    );

    // SummaryPreloader uses shared state for caching

    // Configure summary preloader from config
    let preload_config = final_config.get_summary_preload_config();
    app.set_preload_config(preload_config);

    let (mut monitor_command, mut monitor_rx) = if let Some(cmd) = &final_config.monitor_command {
        let (cmd, rx) =
            AsyncMonitorCommand::new(cmd.clone(), final_config.monitor_interval.unwrap_or(5));
        (Some(cmd), Some(rx))
    } else {
        (None, None)
    };

    // Enable monitor pane when a command is configured
    if monitor_command.is_some() {
        app.toggle_monitor_pane();
        app.set_monitor_command_configured(true);
    }

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let _ = terminal.clear();

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;

    loop {
        // Read git updates from shared state (but skip if a commit is selected to avoid overriding commit files)
        if app.get_selected_commit().is_none() {
            // Check for git updates from shared state
            if let Some(repo) = shared_state_manager.git_state().get_repo() {
                let changed_files = repo.get_display_files();
                let tree = repo.get_file_tree();

                app.update_files(changed_files.clone());
                app.update_tree(&tree);
            }

            // Check for git errors in shared state
            if let Some(error) = shared_state_manager.git_state().get_error("git_status") {
                error!("Git shared state error: {error}");
                // Clear the error after handling it
                shared_state_manager.git_state().clear_error("git_status");
            }
        } else {
            // Check for git errors in shared state
            if let Some(error) = shared_state_manager.git_state().get_error("git_status") {
                error!("Git shared state error: {error}");
                // Clear the error after handling it
                shared_state_manager.git_state().clear_error("git_status");
            }
        }

        // Update monitor command if it exists
        if let Some(ref mut rx) = monitor_rx {
            // Poll for new monitor output
            while let Ok(monitor_output) = rx.try_recv() {
                app.update_monitor_output(monitor_output.output.clone());
                app.update_monitor_timing(Some(monitor_output.timestamp.elapsed()), true);
            }
        }

        // Update timing information
        if let Some(ref monitor) = monitor_command {
            let elapsed = monitor.get_elapsed_since_last_run();
            let has_run = monitor.has_run_yet();
            app.update_monitor_timing(elapsed, has_run);
        }

        // Check shared state for cached summaries
        if let Some(current_commit) = app.get_current_selected_commit_from_picker() {
            if let Some(cached_summary) = shared_state_manager
                .llm_state()
                .get_cached_summary(&current_commit.sha)
            {
                // Handle cached summary from shared state
                app.handle_cached_summary_result(Some(cached_summary), &current_commit.sha);
            }
        }

        // Periodic error recovery check - clear stale errors every 30 seconds
        static LAST_ERROR_CLEANUP: std::sync::atomic::AtomicU64 =
            std::sync::atomic::AtomicU64::new(0);
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let last_cleanup = LAST_ERROR_CLEANUP.load(std::sync::atomic::Ordering::Relaxed);

        if current_time.saturating_sub(last_cleanup) > 30 {
            // Clear any stale errors that might have accumulated
            if shared_state_manager.git_state().has_errors()
                || shared_state_manager.llm_state().has_errors()
            {
                debug!("Performing periodic error cleanup");

                // Only clear errors that are not currently being displayed
                // Git errors are cleared immediately after display, so any remaining are stale
                let git_errors = shared_state_manager.git_state().get_all_errors();
                for (key, _) in git_errors {
                    if key != "git_status" {
                        // Keep current git_status errors
                        shared_state_manager.git_state().clear_error(&key);
                    }
                }

                // Clear old LLM summary errors (keep advice_generation for current display)
                let llm_errors = shared_state_manager.llm_state().get_all_errors();
                for (key, _) in llm_errors {
                    if key.starts_with("summary_") && key != "advice_generation" {
                        shared_state_manager.llm_state().clear_error(&key);
                    }
                }
            }

            LAST_ERROR_CLEANUP.store(current_time, std::sync::atomic::Ordering::Relaxed);
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

            if let Some(repo) = &shared_state_manager.git_state().get_repo() {
                ui::render::<CrosstermBackend<std::io::Stdout>>(f, &app, repo);
            }
        })?;
        let render_duration = render_start.elapsed();

        if render_duration.as_millis() > 10 {
            log::debug!("Slow render detected: {render_duration:?}");
        }

        // Update commit summary pane with current selection from commit picker
        if app.is_in_commit_picker_mode() {
            app.update_commit_summary_with_current_selection(shared_state_manager.llm_state());

            // Trigger continuous pre-loading as user navigates
            if let Some((commits, current_index)) = app.get_commit_picker_state() {
                if !commits.is_empty() {
                    app.preload_summaries_around_index(&commits, current_index);
                }
            }
        }

        // Handle cache callbacks from CommitSummaryPane
        app.handle_commit_summary_cache_callbacks(shared_state_manager.llm_state());

        // Poll for LLM summary updates from shared state
        // Summary updates are now handled through shared state cache

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if handle_key_event(key, &mut app, &final_config, &shared_state_manager) {
                    break;
                }
            }
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
                    debug!(
                        "Processing commit selection: {} - {}",
                        selected_commit.short_sha, selected_commit.message
                    );

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

    // Cleanup shared state
    if let Err(e) = shared_state_manager.shutdown() {
        error!("Error during shared state shutdown: {}", e);
    } else {
        info!("Shared state architecture shutdown completed successfully");
    }

    log::info!("Application shutdown complete");
    Ok(())
}

fn handle_key_event(
    key: KeyEvent,
    app: &mut App,
    config: &Config,
    shared_state_manager: &SharedStateManager,
) -> bool {
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
            KeyCode::Char('?') => {
                debug!("User pressed '?' in commit picker mode, toggling help");
                app.toggle_help();
                return false;
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

    let panes_handled = app.forward_key_to_panes(key);
    if panes_handled {
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
            // This should have been handled by forward_key_to_panes above
            app.scroll_to_bottom(app.current_diff_height);
            false
        }
        KeyCode::Char('j') if key.modifiers.is_empty() => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_down(app.current_diff_height);
            false
        }
        KeyCode::Down => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_down(app.current_diff_height);
            false
        }
        KeyCode::Char('k') if key.modifiers.is_empty() => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_up();
            false
        }
        KeyCode::Up => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_up();
            false
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_down(app.current_diff_height);
            false
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // This should have been handled by forward_key_to_panes above
            app.scroll_up();
            false
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
        KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+W - returning to working directory view");
            app.clear_selected_commit();
            false
        }
        KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            debug!("User pressed Ctrl+P - activating commit picker");
            // Only activate commit picker when in appropriate diff mode
            if app.is_showing_diff_panel() && !app.is_in_commit_picker_mode() {
                if let Some(repo) = shared_state_manager.git_state().get_repo() {
                    // Enter commit picker mode first and show loading state
                    app.enter_commit_picker_mode();
                    app.set_commit_picker_loading();

                    // Create a temporary GitWorker to load commit history using shared state
                    match crate::git_worker::GitWorker::new(
                        repo.path.clone(),
                        Arc::clone(&shared_state_manager.git_state()),
                    ) {
                        Ok(mut git_worker) => {
                            // Configure cache size from config
                            git_worker.set_cache_size(config.get_commit_cache_size());

                            // Use configurable commit history limit
                            let commit_limit = config.get_commit_history_limit();
                            match git_worker.get_commit_history(commit_limit) {
                                Ok(commits) => {
                                    debug!("Successfully loaded {} commits", commits.len());
                                    app.update_commit_picker_commits(commits.clone());

                                    // Configure and trigger summary pre-loading
                                    let preload_config = config.get_summary_preload_config();
                                    app.set_preload_config(preload_config);

                                    // Start pre-loading summaries for the first few commits
                                    debug!(
                                        "Starting summary pre-loading for {} commits",
                                        commits.len()
                                    );
                                    app.preload_summaries(&commits);
                                }
                                Err(e) => {
                                    error!("Failed to load commit history: {}", e);
                                    let error_msg =
                                        if e.to_string().contains("not a git repository") {
                                            "This directory is not a Git repository".to_string()
                                        } else if e.to_string().contains("no commits")
                                            || e.to_string().contains("HEAD")
                                        {
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

// Note: Tests removed during shared state migration
// TODO: Rewrite tests to use new shared state architecture
