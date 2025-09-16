use clap::Parser;
use color_eyre::eyre::Result;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    crossterm::{
        event::DisableMouseCapture,
        event::EnableMouseCapture,
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
        match final_config.theme.unwrap_or(config::Theme::Dark) {
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
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    loop {
        // Poll for git updates
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

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if handle_key_event(key, &mut app) {
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
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;
    let _ = terminal.clear();

    log::info!("Application shutdown complete");
    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App) -> bool {
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
        _ => false,
    }
}
