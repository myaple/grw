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

mod git;
mod ui;

use git::GitRepo;
use ui::App;

include!(concat!(env!("OUT_DIR"), "/git_sha.rs"));

#[derive(Parser)]
#[command(name = "grw")]
#[command(about = "Git Repository Watcher - A TUI for real-time git monitoring")]
#[command(disable_version_flag = true)]
struct Args {
    #[arg(short, long, help = "Print version information and exit")]
    version: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    if args.version {
        println!("grw version 0.1.0 (git: {})", GIT_SHA);
        return Ok(());
    }

    color_eyre::install()?;

    let repo_path = std::env::current_dir()?;
    let mut git_repo = GitRepo::new(repo_path)?;

    let mut app = App::new();

    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;

    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;

    let mut last_update = std::time::Instant::now();
    let update_interval = Duration::from_millis(500);

    loop {
        if last_update.elapsed() >= update_interval {
            git_repo.update()?;
            app.update_files(git_repo.get_changed_files_clone());
            let tree = git_repo.get_file_tree();
            app.update_tree(&tree);
            last_update = std::time::Instant::now();
        }

        terminal.draw(|f| {
            let size = f.area();

            // Calculate layout to get diff panel height
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

            // Store the current diff height in the app for scrolling methods
            app.current_diff_height = diff_height;

            ui::render::<CrosstermBackend<std::io::Stdout>>(f, &app, &git_repo);
        })?;

        if crossterm::event::poll(Duration::from_millis(10))? {
            if let Event::Key(key) = crossterm::event::read()? {
                if handle_key_event(key, &mut app) {
                    break;
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App) -> bool {
    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => true,
        KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
            app.scroll_to_bottom(app.current_diff_height);
            false
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_down(app.current_diff_height);
            false
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_up();
            false
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_down(app.current_diff_height);
            false
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
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
        KeyCode::Char('t') => {
            // Check if g was pressed recently
            if let Some(last_time) = app.last_g_press {
                if std::time::Instant::now()
                    .duration_since(last_time)
                    .as_millis()
                    < 500
                {
                    app.next_file();
                }
            }
            false
        }
        KeyCode::Char('T') => {
            // Check if g was pressed recently
            if let Some(last_time) = app.last_g_press {
                if std::time::Instant::now()
                    .duration_since(last_time)
                    .as_millis()
                    < 500
                {
                    app.prev_file();
                }
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
            app.next_file();
            false
        }
        KeyCode::BackTab => {
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
        _ => false,
    }
}
