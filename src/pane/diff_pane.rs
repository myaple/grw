use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::App;

pub struct DiffPane {
    visible: bool,
}

impl Default for DiffPane {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffPane {
    pub fn new() -> Self {
        Self { visible: true }
    }
}

impl Pane for DiffPane {
    fn title(&self) -> String {
        "Diff".to_string()
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
            let title = format!("Diff: {file_path}");

            let mut lines = Vec::new();

            for (i, line) in file.line_strings.iter().enumerate() {
                if i < app.get_scroll_offset() {
                    continue;
                }

                if lines.len() >= app.current_diff_height {
                    break;
                }

                let (style, line_text) = if line.starts_with('+') {
                    (Style::default().fg(theme.added_color()), line)
                } else if line.starts_with('-') {
                    (Style::default().fg(theme.removed_color()), line)
                } else if line.starts_with(' ') {
                    (Style::default().fg(theme.unchanged_color()), line)
                } else {
                    (Style::default().fg(theme.foreground_color()), line)
                };

                let span = Span::styled(line_text.clone(), style);
                lines.push(Line::from(span));
            }

            let text = ratatui::text::Text::from(lines);
            let paragraph = Paragraph::new(text)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(paragraph, area);
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
