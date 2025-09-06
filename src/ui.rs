use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use crate::git::FileDiff;

#[derive(Debug)]
pub struct App {
    files: Vec<FileDiff>,
    current_file_index: usize,
    scroll_offset: usize,
    max_lines: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            current_file_index: 0,
            scroll_offset: 0,
            max_lines: 20,
        }
    }
    
    pub fn update_files(&mut self, files: Vec<FileDiff>) {
        self.files = files;
        if self.current_file_index >= self.files.len() {
            self.current_file_index = 0;
            self.scroll_offset = 0;
        }
    }
    
    pub fn scroll_down(&mut self) {
        if self.current_file_index < self.files.len() {
            let current_file = &self.files[self.current_file_index];
            if self.scroll_offset + self.max_lines < current_file.line_strings.len() {
                self.scroll_offset += 1;
            }
        }
    }
    
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }
    
    pub fn next_file(&mut self) {
        if !self.files.is_empty() {
            self.current_file_index = (self.current_file_index + 1) % self.files.len();
            self.scroll_offset = 0;
        }
    }
    
    pub fn prev_file(&mut self) {
        if !self.files.is_empty() {
            self.current_file_index = if self.current_file_index == 0 {
                self.files.len() - 1
            } else {
                self.current_file_index - 1
            };
            self.scroll_offset = 0;
        }
    }
    
    pub fn get_current_file(&self) -> Option<&FileDiff> {
        self.files.get(self.current_file_index)
    }
    
    pub fn get_file_count(&self) -> usize {
        self.files.len()
    }
}

pub fn render<B: Backend>(f: &mut Frame, app: &App) {
    let size = f.area();
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
        ])
        .split(size);
    
    render_file_tabs::<B>(f, app, chunks[0]);
    render_diff_view::<B>(f, app, chunks[1]);
}

fn render_file_tabs<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    let tab_items: Vec<ListItem> = app.files.iter().map(|file| {
        let file_name = file.path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Unknown");
        
        let status_char = if file.status.is_wt_new() {
            "N"
        } else if file.status.is_wt_modified() {
            "M"
        } else if file.status.is_wt_deleted() {
            "D"
        } else {
            "?"
        };
        
        let style = if app.files.iter().position(|f| f.path == file.path) == Some(app.current_file_index) {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(Color::Gray)
        };
        
        ListItem::new(Span::styled(format!("{} {}", status_char, file_name), style))
    }).collect();
    
    let tabs = List::new(tab_items)
        .block(Block::default().title("Changed Files").borders(Borders::ALL))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));
    
    f.render_widget(tabs, area);
}

fn render_diff_view<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
    if let Some(file) = app.get_current_file() {
        let file_path = file.path.to_string_lossy();
        let title = format!("Diff: {}", file_path);
        
        let mut lines = Vec::new();
        
        for (i, line) in file.line_strings.iter().enumerate() {
            if i < app.scroll_offset {
                continue;
            }
            
            if lines.len() >= app.max_lines {
                break;
            }
            
            let (style, line_text) = if line.starts_with('+') {
                (Style::default().fg(Color::Green), line)
            } else if line.starts_with('-') {
                (Style::default().fg(Color::Red), line)
            } else if line.starts_with(' ') {
                (Style::default().fg(Color::Gray), line)
            } else {
                (Style::default().fg(Color::White), line)
            };
            
            let span = Span::styled(line_text.clone(), style);
            lines.push(Line::from(span));
        }
        
        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: false });
        
        f.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new("No changes detected")
            .block(Block::default().title("Diff").borders(Borders::ALL));
        f.render_widget(paragraph, area);
    }
}