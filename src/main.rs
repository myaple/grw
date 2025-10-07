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
use ui::{App, ColorPalette};

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
    if let Err(e) = shared_state_manager.initialize() {
        error!("Failed to initialize shared state: {}", e);
        return Err(color_eyre::eyre::eyre!(
            "Failed to initialize shared state: {}",
            e
        ));
    }
    info!("Shared state manager initialized successfully");

    // Create GitWorker with shared state and start it running continuously
    let mut git_worker = crate::git::GitWorker::new(
        repo_path.clone(),
        Arc::clone(shared_state_manager.git_state()),
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

    // Theme setup
    let mut themes = vec![ui::Theme::Dark, ui::Theme::Light];
    if let Some(custom_theme_config) = &final_config.custom_theme {
        let mut custom_palette = ui::ColorPalette::dark(); // Base
        let mut any_color_parsed = false;

        macro_rules! apply_color {
            ($field:ident) => {
                if let Some(hex) = &custom_theme_config.$field {
                    match ui::parse_hex_color(hex) {
                        Ok(color) => {
                            custom_palette.$field = color;
                            any_color_parsed = true;
                        }
                        Err(e) => log::warn!(
                            "Failed to parse custom theme color for '{}': {}",
                            stringify!($field),
                            e
                        ),
                    }
                }
            };
        }

        apply_color!(background);
        apply_color!(foreground);
        apply_color!(primary);
        apply_color!(secondary);
        apply_color!(error);
        apply_color!(highlight);
        apply_color!(border);
        apply_color!(directory);
        apply_color!(added);
        apply_color!(removed);
        apply_color!(unchanged);

        if any_color_parsed {
            themes.push(ui::Theme::Custom(Arc::new(custom_palette)));
        } else {
            log::warn!("Custom theme section found in config, but no valid colors were parsed. Custom theme will not be available.");
        }
    }

    let initial_theme_config = final_config.theme.clone().unwrap_or(config::Theme::Dark);

    let initial_theme_index = match initial_theme_config {
        config::Theme::Dark => 0,
        config::Theme::Light => 1,
        config::Theme::Custom => {
            if themes.iter().any(|t| matches!(t, ui::Theme::Custom(_))) {
                themes.len() - 1 // The last one is custom
            } else {
                log::warn!("Configured theme is 'custom', but no valid custom theme was loaded. Falling back to dark theme.");
                0 // Fallback to dark
            }
        }
    };

    let mut app = App::new_with_config(
        !final_config.no_diff.unwrap_or(false),
        !final_config.hide_changed_files_pane.unwrap_or(false),
        initial_theme_index,
        themes,
        llm_client.clone(),
        Arc::clone(shared_state_manager.llm_state()),
    );

    // SummaryPreloader uses shared state for caching

    // Configure summary preloader from config
    let preload_config = final_config.get_summary_preload_config();
    app.set_preload_config(preload_config);

    let (monitor_command, mut monitor_rx) = if let Some(cmd) = &final_config.monitor_command {
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
        // Read git updates from shared state
        if let Some(repo) = shared_state_manager.git_state().get_repo() {
            use crate::git::ViewMode;

            // Always update files and tree based on current view mode
            let changed_files = repo.get_display_files();
            let tree = repo.get_file_tree();

            log::trace!(
                "Main loop: view_mode={:?}, selected_commit={}, files={}",
                repo.current_view_mode,
                app.get_selected_commit().is_some(),
                changed_files.len()
            );

            // Handle view mode changes automatically, but preserve user-selected commits
            match repo.current_view_mode {
                ViewMode::WorkingTree | ViewMode::Staged | ViewMode::DirtyDirectory => {
                    // Working directory has changes
                    if app.get_selected_commit().is_some() {
                        // User has explicitly selected a commit, preserve it
                        debug!(
                            "Preserving user-selected commit despite working directory changes (mode: {:?})",
                            repo.current_view_mode
                        );
                        // Don't clear the selected commit or update files/tree
                    } else {
                        // No commit selected, show working directory changes
                        app.update_files(changed_files.clone());
                        app.update_tree(&tree);
                    }
                }
                ViewMode::LastCommit => {
                    // Working directory is clean
                    if app.get_selected_commit().is_some() {
                        // User has explicitly selected a commit, preserve it
                        debug!(
                            "Preserving user-selected commit instead of auto-showing last commit"
                        );
                        // Don't override user selection
                    } else if !repo.last_commit_files.is_empty() {
                        // No commit selected, automatically show last commit files
                        debug!("Switching to last commit view");
                        // Update to show last commit files WITHOUT selecting a commit
                        // This ensures we stay in last commit mode, not commit picked mode
                        app.update_files(changed_files.clone());
                        app.update_tree(&tree);
                    }
                }
            }
        }

        // Check for git errors in shared state
        if let Some(error) = shared_state_manager.git_state().get_error("git_status") {
            error!("Git shared state error: {error}");
            // Clear the error after handling it
            shared_state_manager.git_state().clear_error("git_status");
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
        if let Some(current_commit) = app.get_current_selected_commit_from_picker()
            && let Some(cached_summary) = shared_state_manager
                .llm_state()
                .get_cached_summary(&current_commit.sha)
        {
            // Handle cached summary from shared state
            app.handle_cached_summary_result(Some(cached_summary), &current_commit.sha);
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
            log::trace!("Slow render detected: {render_duration:?}");
        }

        // Update commit summary pane with current selection from commit picker
        if app.is_in_commit_picker_mode() {
            app.update_commit_summary_with_current_selection(shared_state_manager.llm_state());

            // Trigger continuous pre-loading as user navigates
            if let Some((commits, current_index)) = app.get_commit_picker_state()
                && !commits.is_empty()
            {
                app.preload_summaries_around_index(&commits, current_index);
            }
        }

        // Handle cache callbacks from CommitSummaryPane
        app.handle_commit_summary_cache_callbacks(shared_state_manager.llm_state());

        // Check for async advice panel task completion
        app.check_advice_panel_tasks();

        // Poll for LLM summary updates from shared state
        // Summary updates are now handled through shared state cache

        if crossterm::event::poll(Duration::from_millis(10))?
            && let Event::Key(key) = crossterm::event::read()?
            && handle_key_event(key, &mut app, &final_config, &shared_state_manager)
        {
            break;
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
    // Handle Ctrl+P commit picker activation separately as it needs access to config and shared_state_manager
    if key.code == KeyCode::Char('p') && key.modifiers.contains(KeyModifiers::CONTROL) {
        debug!("User pressed Ctrl+P - activating commit picker");
        // Only activate commit picker when in appropriate diff mode
        if app.is_showing_diff_panel() && !app.is_in_commit_picker_mode() {
            if let Some(repo) = shared_state_manager.git_state().get_repo() {
                // Enter commit picker mode first and show loading state
                app.enter_commit_picker_mode();
                app.set_commit_picker_loading();
                // Create a temporary GitWorker to load commit history using shared state
                match crate::git::GitWorker::new(
                    repo.path.clone(),
                    Arc::clone(shared_state_manager.git_state()),
                ) {
                    Ok(mut git_worker) => {
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
                                let error_msg = if e.to_string().contains("not a git repository") {
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
                        app.set_commit_picker_error(format!(
                            "Failed to access Git repository: {}",
                            e
                        ));
                    }
                }
            } else {
                app.set_commit_picker_error("No Git repository available".to_string());
            }
        }
        return false;
    }

    // Handle Ctrl+O monitor pane toggle separately as it needs special handling
    if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
        debug!("User pressed Ctrl+O - toggling monitor pane");
        app.toggle_monitor_pane();
        debug!("Monitor pane is now: {}", app.is_showing_monitor_pane());
        return false;
    }

    // Handle Ctrl+W returning to working directory view separately
    // Only handle if advice panel chat input is NOT active
    if key.code == KeyCode::Char('w')
        && key.modifiers.contains(KeyModifiers::CONTROL)
        && !app.is_advice_panel_chat_input_active()
    {
        debug!("User pressed Ctrl+W - returning to working directory view");
        app.clear_selected_commit();
        return false;
    }

    // Use the new keys module for all other key handling
    match pane::GlobalKeyHandler::handle_global_key(app, &key) {
        pane::KeyResult::Quit => true,
        pane::KeyResult::Handled => false,
        pane::KeyResult::NotHandled => false,
    }
}

// Note: Tests removed during shared state migration
