use color_eyre::eyre::Result;
use ratatui::{
    backend::CrosstermBackend,
    crossterm::{
        event::{Event, KeyCode, KeyEvent, KeyModifiers},
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    },
    Terminal,
};
use std::io;
use std::time::Duration;

mod git;
mod ui;

use git::GitRepo;
use ui::App;

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;
    
    let repo_path = std::env::current_dir()?;
    let mut git_repo = GitRepo::new(repo_path)?;
    
    let mut app = App::new();
    
    let backend = CrosstermBackend::new(io::stdout());
    let mut terminal = Terminal::new(backend)?;
    
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    
    let mut last_update = std::time::Instant::now();
    let update_interval = Duration::from_millis(500);
    
    loop {
        if last_update.elapsed() >= update_interval {
            git_repo.update()?;
            app.update_files(git_repo.get_changed_files_clone());
            last_update = std::time::Instant::now();
        }
        
        terminal.draw(|f| {
            ui::render::<CrosstermBackend<std::io::Stdout>>(f, &app);
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
    execute!(io::stdout(), LeaveAlternateScreen)?;
    
    Ok(())
}

fn handle_key_event(key: KeyEvent, app: &mut App) -> bool {
    match key.code {
        KeyCode::Char('q') => true,
        KeyCode::Char('j') | KeyCode::Down => {
            app.scroll_down();
            false
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.scroll_up();
            false
        }
        KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_down();
            false
        }
        KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll_up();
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
        KeyCode::Char('h') | KeyCode::Left => {
            app.prev_file();
            false
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.next_file();
            false
        }
        _ => false,
    }
}
