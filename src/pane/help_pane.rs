use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::{ActivePane, App};

pub struct HelpPane {
    visible: bool,
}

impl Default for HelpPane {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpPane {
    pub fn new() -> Self {
        Self { visible: false }
    }
}

impl Pane for HelpPane {
    fn title(&self) -> String {
        "Help".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();
        let last_active_pane = app.get_last_active_pane();

        let mut help_text = vec![
            Line::from(Span::styled(
                "Git Repository Watcher - Help",
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Check if we're in commit picker mode and show commit picker shortcuts
        let (pane_title, pane_hotkeys) = if app.is_in_commit_picker_mode() {
            (
                "Commit Picker",
                vec![
                    "  j / k / ↑ / ↓     - Navigate commits",
                    "  g t               - Next commit",
                    "  g T               - Previous commit",
                    "  Enter             - Select commit",
                    "  Esc               - Exit commit picker",
                    "  Ctrl+P            - Enter commit picker mode",
                    "  Ctrl+W            - Return to working directory",
                ],
            )
        } else {
            match last_active_pane {
                ActivePane::FileTree => (
                    "File Tree",
                    vec![
                        "  Tab / g t     - Next file",
                        "  Shift+Tab / g T - Previous file",
                    ],
                ),
                ActivePane::Monitor => (
                    "Monitor",
                    vec![
                        "  Alt+j / Alt+Down  - Scroll down",
                        "  Alt+k / Alt+Up    - Scroll up",
                    ],
                ),
                ActivePane::Diff | ActivePane::SideBySideDiff => (
                    "Diff View",
                    vec![
                        "  j / Down / Ctrl+e - Scroll down",
                        "  k / Up / Ctrl+y   - Scroll up",
                        "  PageDown          - Page down",
                        "  PageUp            - Page up",
                        "  g g               - Go to top",
                        "  Shift+G           - Go to bottom",
                    ],
                ),
            }
        };

        help_text.push(Line::from(Span::styled(
            format!("{pane_title} Hotkeys:"),
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )));
        for hotkey in pane_hotkeys {
            help_text.push(Line::from(hotkey));
        }
        help_text.push(Line::from(""));

        help_text.extend(vec![
            Line::from(Span::styled(
                "General:",
                Style::default()
                    .fg(theme.primary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  ?             - Show/hide this help page"),
            Line::from("  Esc           - Exit help page"),
            Line::from("  Ctrl+h        - Toggle diff panel visibility"),
            Line::from("  Ctrl+o        - Toggle monitor pane visibility"),
            Line::from("  Ctrl+t        - Toggle light/dark theme"),
            Line::from("  q / Ctrl+c    - Quit application"),
        ]);

        // Add commit picker shortcut if not already in commit picker mode
        if !app.is_in_commit_picker_mode() {
            help_text.push(Line::from("  Ctrl+P        - Enter commit picker mode"));
        }

        // Add working directory shortcut if we have a selected commit
        if app.get_selected_commit().is_some() {
            help_text.push(Line::from("  Ctrl+W        - Return to working directory"));
        }

        help_text.extend(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Pane Modes:",
                Style::default()
                    .fg(theme.primary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Ctrl+d        - Switch to inline diff view"),
            Line::from("  Ctrl+s        - Switch to side-by-side diff view"),
            Line::from(""),
            Line::from("Press ? or Esc to return to the previous pane"),
        ]);

        let text = ratatui::text::Text::from(help_text);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(self.title())
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
                KeyCode::Char('?') => {
                    self.set_visible(false);
                    true
                }
                KeyCode::Esc => {
                    self.set_visible(false);
                    true
                }
                _ => false,
            },
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
