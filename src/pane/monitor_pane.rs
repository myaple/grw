use crossterm::event::{KeyCode, KeyModifiers};
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

pub struct MonitorPane {
    visible: bool,
    scroll_offset: usize,
    output: String,
}

impl Default for MonitorPane {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
            output: String::new(),
        }
    }

    pub fn update_output(&mut self, output: String) {
        self.output = output;
    }
}

impl Pane for MonitorPane {
    fn title(&self) -> String {
        "Monitor".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();
        let monitor_lines: Vec<_> = self.output.lines().skip(self.scroll_offset).collect();
        let visible_lines = area.height.saturating_sub(2) as usize;

        let display_lines: Vec<Line> = monitor_lines
            .iter()
            .take(visible_lines)
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme.foreground_color()),
                ))
            })
            .collect();

        let title = if !app.get_monitor_command_configured() {
            "Monitor (no command configured)".to_string()
        } else if !app.get_monitor_has_run() {
            "Monitor ⏳ loading...".to_string()
        } else if let Some(elapsed) = app.get_monitor_elapsed_time() {
            let time_str = app.format_elapsed_time(elapsed);
            format!("Monitor ⏱️ {time_str} ago")
        } else {
            "Monitor Output".to_string()
        };

        let text = ratatui::text::Text::from(display_lines);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => match key.code {
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    true
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    true
                }
                _ => false,
            },
            AppEvent::DataUpdated(_, data) => {
                self.update_output(data.clone());
                true
            }
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
