use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::App;

pub struct SideBySideDiffPane {
    visible: bool,
}

impl Default for SideBySideDiffPane {
    fn default() -> Self {
        Self::new()
    }
}

impl SideBySideDiffPane {
    pub fn new() -> Self {
        Self { visible: false }
    }
}

impl Pane for SideBySideDiffPane {
    fn title(&self) -> String {
        "Side-by-side Diff".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();
        if let Some(file) = app.get_current_file() {
            let file_path = file.path.to_string_lossy();
            let _title = format!("Side-by-side Diff: {file_path}");

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            let mut left_lines = Vec::new();
            let mut right_lines = Vec::new();

            let mut line_count = 0;
            for (i, line) in file.line_strings.iter().enumerate() {
                if i < app.get_scroll_offset() {
                    continue;
                }

                if line_count >= app.current_diff_height {
                    break;
                }

                let (left_content, right_content) = if let Some(stripped) = line.strip_prefix('+') {
                    ("".to_string(), stripped.to_string())
                } else if let Some(stripped) = line.strip_prefix('-') {
                    (stripped.to_string(), "".to_string())
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    let content = stripped.to_string();
                    (content.clone(), content)
                } else {
                    (line.to_string(), line.to_string())
                };

                let left_style = if line.starts_with('-') {
                    Style::default().fg(theme.removed_color())
                } else if line.starts_with(' ') || line.starts_with('+') {
                    Style::default().fg(theme.unchanged_color())
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                let right_style = if line.starts_with('+') {
                    Style::default().fg(theme.added_color())
                } else if line.starts_with(' ') || line.starts_with('-') {
                    Style::default().fg(theme.unchanged_color())
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                left_lines.push(Line::from(Span::styled(left_content, left_style)));
                right_lines.push(Line::from(Span::styled(right_content, right_style)));

                line_count += 1;
            }

            let left_text = ratatui::text::Text::from(left_lines);
            let right_text = ratatui::text::Text::from(right_lines);

            let left_paragraph = Paragraph::new(left_text)
                .block(
                    Block::default()
                        .title("Original")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            let right_paragraph = Paragraph::new(right_text)
                .block(
                    Block::default()
                        .title("Modified")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(left_paragraph, chunks[0]);
            f.render_widget(right_paragraph, chunks[1]);
        } else {
            let paragraph = Paragraph::new("No changes detected").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
        }
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => matches!(key.code, KeyCode::Char('j') | KeyCode::Char('k')),
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
