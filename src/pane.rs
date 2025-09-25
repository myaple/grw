use std::cell::RefCell;
use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use openai_api_rs::v1::chat_completion;
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};


use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::ui::{ActivePane, App, Theme};
use crate::shared_state::LlmSharedState;
use std::sync::Arc;

pub trait Pane {
    fn title(&self) -> String;
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_event(&mut self, event: &AppEvent) -> bool;
    fn visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn as_advice_pane(&self) -> Option<&AdvicePane> {
        None
    }
    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePane> {
        None
    }
    fn as_commit_picker_pane(&self) -> Option<&CommitPickerPane> {
        None
    }
    fn as_commit_picker_pane_mut(&mut self) -> Option<&mut CommitPickerPane> {
        None
    }
    fn as_commit_summary_pane(&self) -> Option<&CommitSummaryPane> {
        None
    }
    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    FileTree,
    Monitor,
    Diff,
    SideBySideDiff,
    Help,
    StatusBar,
    Advice,
    CommitPicker,
    CommitSummary,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    DataUpdated((), String),
    ThemeChanged(()),
}

pub struct PaneRegistry {
    panes: HashMap<PaneId, Box<dyn Pane>>,
    theme: Theme,
}

impl std::fmt::Debug for PaneRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneRegistry")
            .field("pane_count", &self.panes.len())
            .field("theme", &self.theme)
            .finish()
    }
}

impl PaneRegistry {
    pub fn new(theme: Theme, llm_client: LlmClient, llm_shared_state: Arc<LlmSharedState>) -> Self {
        let mut registry = Self {
            panes: HashMap::new(),
            theme,
        };

        registry.register_default_panes(llm_client, llm_shared_state);
        registry
    }

    fn register_default_panes(&mut self, llm_client: LlmClient, llm_shared_state: Arc<LlmSharedState>) {
        self.register_pane(PaneId::FileTree, Box::new(FileTreePane::new()));
        self.register_pane(PaneId::Monitor, Box::new(MonitorPane::new()));
        self.register_pane(PaneId::Diff, Box::new(DiffPane::new()));
        self.register_pane(PaneId::SideBySideDiff, Box::new(SideBySideDiffPane::new()));
        self.register_pane(PaneId::Help, Box::new(HelpPane::new()));
        self.register_pane(PaneId::StatusBar, Box::new(StatusBarPane::new()));
        self.register_pane(
            PaneId::Advice,
            Box::new(AdvicePane::new(Some(llm_client.clone()))),
        );
        self.register_pane(PaneId::CommitPicker, Box::new(CommitPickerPane::new()));
        let mut commit_summary_pane = CommitSummaryPane::new_with_llm_client(Some(llm_client));
        commit_summary_pane.set_shared_state(llm_shared_state);
        self.register_pane(
            PaneId::CommitSummary,
            Box::new(commit_summary_pane),
        );
    }

    pub fn register_pane(&mut self, id: PaneId, pane: Box<dyn Pane>) {
        self.panes.insert(id, pane);
    }

    pub fn get_pane(&self, id: &PaneId) -> Option<&dyn Pane> {
        self.panes.get(id).map(|p| p.as_ref())
    }

    pub fn with_pane_mut<F, R>(&mut self, id: &PaneId, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Pane) -> R,
    {
        self.panes.get_mut(id).map(|p| f(p.as_mut()))
    }

    pub fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        pane_id: PaneId,
        git_repo: &GitRepo,
    ) {
        if let Some(pane) = self.get_pane(&pane_id)
            && pane.visible()
            && let Err(e) = pane.render(f, app, area, git_repo)
        {
            log::error!("Error rendering pane {pane_id:?}: {e}");
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        // Notify all panes of theme change
        let event = AppEvent::ThemeChanged(());
        for pane in self.panes.values_mut() {
            let _ = pane.handle_event(&event);
        }
    }
}

// File Tree Pane Implementation
pub struct FileTreePane {
    visible: bool,
    scroll_offset: usize,
}

impl FileTreePane {
    pub fn new() -> Self {
        Self {
            visible: true,
            scroll_offset: 0,
        }
    }
}

impl Pane for FileTreePane {
    fn title(&self) -> String {
        "Changed Files".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{List, ListItem},
        };

        let theme = app.get_theme();
        let tree_items: Vec<ListItem> = app
            .get_tree_nodes()
            .iter()
            .enumerate()
            .map(|(index, (node, depth))| {
                let indent = "  ".repeat(*depth);
                let name_spans = if node.is_dir {
                    vec![Span::raw(format!("{}📁 {}", indent, node.name))]
                } else {
                    let mut spans = Vec::new();

                    // Add arrow for current file selection
                    if index == app.get_current_tree_index() {
                        spans.push(Span::styled(
                            "-> ",
                            Style::default()
                                .fg(theme.secondary_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::raw("   "));
                    }

                    let status_char = if let Some(ref diff) = node.file_diff {
                        if diff.status.is_wt_new() {
                            "📄 "
                        } else if diff.status.is_wt_modified() {
                            "📝 "
                        } else if diff.status.is_wt_deleted() {
                            "🗑️  "
                        } else {
                            "📄 "
                        }
                    } else {
                        "📄 "
                    };

                    spans.push(Span::raw(format!("{indent}{status_char}")));
                    spans.push(Span::raw(node.name.clone()));

                    if let Some(ref diff) = node.file_diff {
                        if diff.additions > 0 {
                            spans.push(Span::styled(
                                format!(" (+{})", diff.additions),
                                Style::default()
                                    .fg(theme.added_color())
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                        if diff.deletions > 0 {
                            spans.push(Span::styled(
                                format!(" (-{})", diff.deletions),
                                Style::default()
                                    .fg(theme.removed_color())
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    spans
                };

                let line_style = if let Some(ref diff) = node.file_diff {
                    // Check if this file is recently changed by finding its index
                    if let Some(file_idx) = app.get_files().iter().position(|f| f.path == diff.path)
                    {
                        if file_idx < app.get_file_change_timestamps().len()
                            && app.is_file_recently_changed(file_idx)
                        {
                            // Recently changed - highlight
                            Style::default()
                                .fg(theme.foreground_color())
                                .bg(theme.highlight_color())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            // Not recently changed - normal
                            Style::default().fg(theme.foreground_color())
                        }
                    } else {
                        // File not found in files list - normal
                        Style::default().fg(theme.foreground_color())
                    }
                } else {
                    // Directory
                    Style::default()
                        .fg(theme.directory_color())
                        .add_modifier(Modifier::BOLD)
                };

                let line = Line::from(name_spans).style(line_style);
                ListItem::new(line)
            })
            .collect();

        let file_list = List::new(tree_items)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .highlight_style(
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(file_list, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => {
                // Handle key events for file tree navigation
                match key.code {
                    KeyCode::Char('j') => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                        true
                    }
                    KeyCode::Char('k') => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        true
                    }
                    _ => false,
                }
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

// Monitor Pane Implementation
pub struct MonitorPane {
    visible: bool,
    scroll_offset: usize,
    output: String,
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
        use ratatui::{
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };
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

// Diff Pane Implementation
pub struct DiffPane {
    visible: bool,
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
        use ratatui::{
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

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

// Side-by-side Diff Pane Implementation
pub struct SideBySideDiffPane {
    visible: bool,
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
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

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

// Help Pane Implementation
pub struct HelpPane {
    visible: bool,
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
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

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
                ActivePane::Advice => (
                    "LLM Advice",
                    vec![
                        "  j / k           - Scroll up/down",
                        "  g g             - Go to top",
                        "  Shift+G         - Go to bottom",
                        "  /               - Enter input mode",
                        "  Enter           - Submit question",
                        "  Esc             - Exit input mode",
                        "  Ctrl+r          - Refresh LLM advice",
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
            Line::from("  Ctrl+l        - Switch to LLM advice pane"),
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

#[cfg(test)]
mod help_tests {
    use super::*;
    use crate::ui::{App, Theme};
    use std::sync::Arc;
    
    fn create_test_llm_state() -> Arc<crate::shared_state::LlmSharedState> {
        Arc::new(crate::shared_state::LlmSharedState::new())
    }
    use crate::git::CommitInfo;

    #[test]
    fn test_help_detects_commit_picker_mode() {
        let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());
        
        // Test normal mode
        assert!(!app.is_in_commit_picker_mode());
        
        // Enter commit picker mode
        app.enter_commit_picker_mode();
        assert!(app.is_in_commit_picker_mode());
        
        // Exit commit picker mode
        app.exit_commit_picker_mode();
        assert!(!app.is_in_commit_picker_mode());
    }

    #[test]
    fn test_help_detects_selected_commit() {
        let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());
        
        // Initially no commit selected
        assert!(app.get_selected_commit().is_none());
        
        // Create a test commit and select it
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };
        app.select_commit(test_commit);
        
        // Now should have a selected commit
        assert!(app.get_selected_commit().is_some());
        
        // Clear the selected commit
        app.clear_selected_commit();
        assert!(app.get_selected_commit().is_none());
    }

    #[test]
    fn test_commit_summary_pane_cached_summary() {
        let mut pane = CommitSummaryPane::new_with_llm_client(None);
        
        // Create a test commit
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };
        
        // Update with commit
        pane.update_commit(Some(test_commit));
        
        // Initially should need summary
        assert!(pane.needs_summary());
        assert!(pane.llm_summary.is_none());
        
        // Set a cached summary
        pane.set_cached_summary("abc123", "This is a cached summary".to_string());
        
        // Should no longer need summary and should have the cached one
        assert!(!pane.needs_summary());
        assert_eq!(pane.llm_summary, Some("This is a cached summary".to_string()));
        assert_eq!(pane.loading_state, CommitSummaryLoadingState::Loaded);
    }

    #[test]
    fn test_commit_summary_pane_cache_callback() {
        let mut pane = CommitSummaryPane::new_with_llm_client(None);
        
        // Initially no cache callback
        assert!(pane.take_cache_callback().is_none());
        
        // Simulate receiving an LLM response using shared state
        
        // Create a test commit to match the response
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };
        pane.update_commit(Some(test_commit));
        pane.is_loading_summary = true; // Simulate loading state
        pane.pending_summary_sha = Some("abc123".to_string());
        
        // Poll for the summary
        pane.poll_llm_summary();
        
        // Should have a cache callback now
        let cache_callback = pane.take_cache_callback();
        assert!(cache_callback.is_some());
        let (commit_sha, summary) = cache_callback.unwrap();
        assert_eq!(commit_sha, "abc123");
        assert_eq!(summary, "Generated summary");
        
        // Taking again should return None
        assert!(pane.take_cache_callback().is_none());
    }

    #[test]
    fn test_commit_summary_pane_caches_all_summaries() {
        let mut pane = CommitSummaryPane::new_with_llm_client(None);
        
        // Set current commit to "commit1"
        let current_commit = CommitInfo {
            sha: "commit1".to_string(),
            short_sha: "commit1".to_string(),
            message: "Current commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };
        pane.update_commit(Some(current_commit));
        
        // Simulate receiving an LLM response for a DIFFERENT commit (commit2) using shared state
        
        // Poll for the summary
        pane.poll_llm_summary();
        
        // Should have a cache callback even though it's for a different commit
        let cache_callback = pane.take_cache_callback();
        assert!(cache_callback.is_some());
        let (commit_sha, summary) = cache_callback.unwrap();
        assert_eq!(commit_sha, "commit2");
        assert_eq!(summary, "Summary for commit2");
        
        // The UI should NOT be updated since it's for a different commit
        assert!(pane.llm_summary.is_none());
        assert_eq!(pane.loading_state, CommitSummaryLoadingState::Loaded);
    }
}

// Status Bar Pane Implementation
pub struct StatusBarPane {
    visible: bool,
}

impl StatusBarPane {
    pub fn new() -> Self {
        Self { visible: true }
    }
}

impl Pane for StatusBarPane {
    fn title(&self) -> String {
        "".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();
        let repo_name = &git_repo.repo_name;
        let branch = &git_repo.branch_name;
        let (commit_sha, commit_summary) = &git_repo.commit_info;
        let (total_files, total_additions, total_deletions) = git_repo.total_stats;
        let view_mode = git_repo.current_view_mode;

        let view_mode_text = if let Some(selected_commit) = app.get_selected_commit() {
            format!("🔍 Selected Commit: {}", selected_commit.short_sha)
        } else {
            match view_mode {
                crate::git::ViewMode::WorkingTree => "💼 Working Tree".to_string(),
                crate::git::ViewMode::Staged => "📋 Staged Files".to_string(),
                crate::git::ViewMode::DirtyDirectory => "🗂️ Dirty Directory".to_string(),
                crate::git::ViewMode::LastCommit => "📜 Last Commit".to_string(),
            }
        };

        let status_text = if let Some(selected_commit) = app.get_selected_commit() {
            format!(
                "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {} > {} | 📊 {} files (+{}/-{}) | Press Ctrl+W to return to working directory",
                selected_commit.short_sha,
                selected_commit.message.lines().next().unwrap_or(""),
                selected_commit.files_changed.len(),
                selected_commit
                    .files_changed
                    .iter()
                    .map(|f| f.additions)
                    .sum::<usize>(),
                selected_commit
                    .files_changed
                    .iter()
                    .map(|f| f.deletions)
                    .sum::<usize>()
            )
        } else {
            format!(
                "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {commit_sha} > {commit_summary} | 📊 {total_files} files (+{total_additions}/-{total_deletions})"
            )
        };

        let paragraph = Paragraph::new(status_text)
            .style(
                Style::default()
                    .fg(theme.foreground_color())
                    .bg(theme.background_color())
                    .add_modifier(Modifier::REVERSED),
            )
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
        Ok(())
    }

    fn handle_event(&mut self, _event: &AppEvent) -> bool {
        false
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Commit Picker Pane Implementation
pub struct CommitPickerPane {
    visible: bool,
    commits: Vec<crate::git::CommitInfo>,
    current_index: usize,
    scroll_offset: usize,
    last_g_press: Option<std::time::Instant>,
    enter_pressed: bool,
    loading_state: CommitPickerLoadingState,
    error_message: Option<String>,
    // Performance optimization fields
    last_visible_height: usize,
    render_cache_valid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitPickerLoadingState {
    NotLoaded,
    Loading,
    Loaded,
    Error,
}

impl CommitPickerPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            commits: Vec::new(),
            current_index: 0,
            scroll_offset: 0,
            last_g_press: None,
            enter_pressed: false,
            loading_state: CommitPickerLoadingState::NotLoaded,
            error_message: None,
            last_visible_height: 0,
            render_cache_valid: false,
        }
    }

    pub fn set_loading(&mut self) {
        self.loading_state = CommitPickerLoadingState::Loading;
        self.error_message = None;
    }

    pub fn update_commits(&mut self, commits: Vec<crate::git::CommitInfo>) {
        self.commits = commits;
        if self.current_index >= self.commits.len() {
            self.current_index = 0;
            self.scroll_offset = 0;
        }

        // Update loading state based on results
        if self.commits.is_empty() {
            self.loading_state = CommitPickerLoadingState::Loaded;
            // Don't set error for empty repos, just show appropriate message
        } else {
            self.loading_state = CommitPickerLoadingState::Loaded;
            self.error_message = None;
        }

        // Invalidate render cache when commits change
        self.render_cache_valid = false;
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_state = CommitPickerLoadingState::Error;
        self.error_message = Some(error);
        self.commits.clear();
        self.current_index = 0;
        self.scroll_offset = 0;
    }

    pub fn is_loading(&self) -> bool {
        matches!(self.loading_state, CommitPickerLoadingState::Loading)
    }

    pub fn has_error(&self) -> bool {
        matches!(self.loading_state, CommitPickerLoadingState::Error)
    }

    pub fn get_current_commit(&self) -> Option<&crate::git::CommitInfo> {
        // Only return commit if we're in a valid state
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.commits.get(self.current_index)
        } else {
            None
        }
    }

    fn navigate_next(&mut self) {
        // Only allow navigation if commits are loaded and available
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.current_index = (self.current_index + 1) % self.commits.len();
            self.update_scroll_offset(20); // Use reasonable default
        }
    }

    fn navigate_prev(&mut self) {
        // Only allow navigation if commits are loaded and available
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.current_index = if self.current_index == 0 {
                self.commits.len() - 1
            } else {
                self.current_index - 1
            };
            self.update_scroll_offset(20); // Use reasonable default
        }
    }

    fn update_scroll_offset(&mut self, visible_height: usize) {
        // Ensure current selection is visible
        if self.current_index < self.scroll_offset {
            self.scroll_offset = self.current_index;
            self.render_cache_valid = false;
        } else if self.current_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.current_index.saturating_sub(visible_height - 1);
            self.render_cache_valid = false;
        }

        // Update last visible height for performance tracking
        if self.last_visible_height != visible_height {
            self.last_visible_height = visible_height;
            self.render_cache_valid = false;
        }
    }

    pub fn is_enter_pressed(&self) -> bool {
        self.enter_pressed
    }

    pub fn reset_enter_pressed(&mut self) {
        self.enter_pressed = false;
    }

    pub fn get_commits(&self) -> Vec<crate::git::CommitInfo> {
        self.commits.clone()
    }

    pub fn get_current_index(&self) -> usize {
        self.current_index
    }
}

impl Pane for CommitPickerPane {
    fn title(&self) -> String {
        "Commit History".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, List, ListItem, Paragraph},
        };

        let theme = app.get_theme();

        // Handle different loading states
        match self.loading_state {
            CommitPickerLoadingState::NotLoaded => {
                let paragraph = Paragraph::new("Press Ctrl+P to load commit history").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Loading => {
                let paragraph = Paragraph::new("⏳ Loading commit history...").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Error => {
                let error_text = if let Some(error) = &self.error_message {
                    format!("❌ Error loading commits:\n{}", error)
                } else {
                    "❌ Error loading commits".to_string()
                };

                let paragraph = Paragraph::new(error_text)
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Loaded => {
                if self.commits.is_empty() {
                    let paragraph = Paragraph::new("📭 No commits found in this repository\n\nThis might be a new repository with no commits yet.")
                        .block(
                            Block::default()
                                .title(self.title())
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.border_color())),
                        )
                        .style(Style::default().fg(theme.secondary_color()));
                    f.render_widget(paragraph, area);
                    return Ok(());
                }
            }
        }

        // Calculate visible range based on scroll offset and area height
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let start_index = self.scroll_offset;
        let end_index = (start_index + visible_height).min(self.commits.len());

        // Early return if we have no commits to render
        if start_index >= self.commits.len() {
            let paragraph = Paragraph::new("No commits to display").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
            return Ok(());
        }

        let commit_items: Vec<ListItem> = self
            .commits
            .iter()
            .enumerate()
            .skip(start_index)
            .take(end_index - start_index)
            .map(|(original_index, commit)| {
                let mut spans = Vec::new();

                // Add arrow for current selection (use original index for comparison)
                if original_index == self.current_index {
                    spans.push(Span::styled(
                        "-> ",
                        Style::default()
                            .fg(theme.secondary_color())
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }

                // Add short SHA
                spans.push(Span::styled(
                    format!("{} ", commit.short_sha),
                    Style::default()
                        .fg(theme.primary_color())
                        .add_modifier(Modifier::BOLD),
                ));

                // Add first line of commit message
                let first_line = commit.message.lines().next().unwrap_or("").to_string();
                spans.push(Span::styled(
                    first_line,
                    Style::default().fg(theme.foreground_color()),
                ));

                let line_style = if original_index == self.current_index {
                    Style::default()
                        .fg(theme.foreground_color())
                        .bg(theme.highlight_color())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                let line = Line::from(spans).style(line_style);
                ListItem::new(line)
            })
            .collect();

        let commit_list = List::new(commit_items)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .highlight_style(
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(commit_list, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => {
                match key.code {
                    KeyCode::Char('j') => {
                        self.navigate_next();
                        true
                    }
                    KeyCode::Char('k') => {
                        self.navigate_prev();
                        true
                    }
                    KeyCode::Down => {
                        self.navigate_next();
                        true
                    }
                    KeyCode::Up => {
                        self.navigate_prev();
                        true
                    }
                    KeyCode::Char('g') => {
                        self.last_g_press = Some(std::time::Instant::now());
                        true
                    }
                    KeyCode::Char('t') => {
                        // Check if g was pressed recently for g+t navigation
                        if let Some(last_time) = self.last_g_press
                            && std::time::Instant::now()
                                .duration_since(last_time)
                                .as_millis()
                                < 500
                        {
                            self.navigate_next();
                            self.last_g_press = None; // Reset after use
                        }
                        true
                    }
                    KeyCode::Char('T') => {
                        // Check if g was pressed recently for g+T navigation
                        if let Some(last_time) = self.last_g_press
                            && std::time::Instant::now()
                                .duration_since(last_time)
                                .as_millis()
                                < 500
                        {
                            self.navigate_prev();
                            self.last_g_press = None; // Reset after use
                        }
                        true
                    }
                    KeyCode::Enter => {
                        // Only allow commit selection if we have valid commits loaded
                        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
                            && !self.commits.is_empty()
                            && self.current_index < self.commits.len()
                        {
                            self.enter_pressed = true;
                        }
                        true
                    }
                    _ => false,
                }
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

    fn as_commit_picker_pane(&self) -> Option<&CommitPickerPane> {
        Some(self)
    }

    fn as_commit_picker_pane_mut(&mut self) -> Option<&mut CommitPickerPane> {
        Some(self)
    }
}

const SYSTEM_PROMPT: &str = "You are acting in the role of a staff engineer providing a code review. \
Please provide a brief review of the following code changes. \
The review should focus on 'Maintainability' and any obvious safety bugs. \
In the maintainability part, include 0-3 actionable suggestions to enhance code maintainability. \
Don't be afraid to say that this code is okay at maintainability and not provide suggestions. \
When you provide suggestions, give a brief before and after example using the code diffs below \
to provide context and examples of what you mean. \
Each suggestion should be clear, specific, and implementable. \
Keep the response concise and focused on practical improvements.";

// Advice Pane Implementation
pub struct AdvicePane {
    visible: bool,
    content: String,
    scroll_offset: usize,
    input: String,
    input_mode: bool,
    conversation_history: Vec<chat_completion::ChatCompletionMessage>,
    llm_client: Option<LlmClient>,
    is_loading: bool,
    input_cursor_position: usize,
    input_scroll_offset: usize,
    initial_data: Option<String>,
    pub refresh_requested: bool,
    last_rect: RefCell<Rect>,
    last_g_press: Option<std::time::Instant>,
}

impl AdvicePane {
    pub fn new(llm_client: Option<LlmClient>) -> Self {
        // Initialize with a default rect to prevent issues
        let default_rect = Rect::new(0, 0, 80, 20);
        Self {
            visible: false,
            content: "⏳ Loading LLM advice...".to_string(),
            scroll_offset: 0,
            input: String::new(),
            input_mode: false,
            conversation_history: Vec::new(),
            llm_client,
            is_loading: false,
            input_cursor_position: 0,
            input_scroll_offset: 0,
            initial_data: None,
            refresh_requested: false,
            last_rect: RefCell::new(default_rect),
            last_g_press: None,
        }
    }

    pub fn poll_llm_response(&mut self) {
        // This method is now deprecated - use shared state instead
        self.is_loading = false;
    }
}

impl Pane for AdvicePane {
    fn title(&self) -> String {
        "LLM Advice".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        *self.last_rect.borrow_mut() = area;
        let theme = app.get_theme();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(0),
                    Constraint::Length(if self.input_mode { 3 } else { 0 }),
                ]
                .as_ref(),
            )
            .split(area);

        let mut text_lines: Vec<Line> = self
            .content
            .lines()
            .map(|l| Line::from(l.to_string()))
            .collect();
        if self.is_loading {
            text_lines.push(Line::from("Loading..."));
        }

        let paragraph = Paragraph::new(text_lines)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));
        f.render_widget(paragraph, chunks[0]);

        if self.input_mode {
            let input_block = Block::default().borders(Borders::ALL).title("Input");
            let input_paragraph = Paragraph::new(&*self.input)
                .style(Style::default().fg(theme.foreground_color()))
                .scroll((0, self.input_scroll_offset as u16))
                .block(input_block);
            f.render_widget(input_paragraph, chunks[1]);
            f.set_cursor_position(ratatui::layout::Position::new(
                chunks[1].x + (self.input_cursor_position - self.input_scroll_offset) as u16 + 1,
                chunks[1].y + 1,
            ));
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        if !self.visible {
            return false;
        }
        if let AppEvent::Key(key) = event {
            if self.input_mode {
                match key.code {
                    KeyCode::Esc => {
                        self.input_mode = false;
                        return true;
                    }
                    KeyCode::Enter => {
                        let prompt = self.input.drain(..).collect::<String>();
                        self.conversation_history
                            .push(chat_completion::ChatCompletionMessage {
                                role: chat_completion::MessageRole::user,
                                content: chat_completion::Content::Text(prompt.clone()),
                                name: None,
                                tool_calls: None,
                                tool_call_id: None,
                            });

                        self.content.push_str("\n\n> ");
                        self.content.push_str(&prompt);
                        self.is_loading = true;
                        self.input_mode = false;

                        // Scroll to bottom
                        let content_lines: Vec<_> = self.content.lines().collect();
                        self.scroll_offset = content_lines.len().saturating_sub(1);

                        if let Some(_llm_client) = self.llm_client.as_ref() {
                            // TODO: Implement LLM advice generation using shared state
                            self.content.push_str("\n\nLLM advice generation not yet implemented with shared state");
                        }
                        self.input_cursor_position = 0;
                        return true;
                    }
                    KeyCode::Char(c) => {
                        self.input.insert(self.input_cursor_position, c);
                        self.input_cursor_position += 1;
                        return true;
                    }
                    KeyCode::Backspace => {
                        if self.input_cursor_position > 0 {
                            self.input_cursor_position -= 1;
                            self.input.remove(self.input_cursor_position);
                        }
                        return true;
                    }
                    KeyCode::Left => {
                        self.input_cursor_position = self.input_cursor_position.saturating_sub(1);
                        return true;
                    }
                    KeyCode::Right => {
                        self.input_cursor_position = self
                            .input_cursor_position
                            .saturating_add(1)
                            .min(self.input.len());
                        return true;
                    }
                    _ => return false,
                }
            } else {
                match key.code {
                    KeyCode::Char('/') => {
                        self.input_mode = true;
                        if self.conversation_history.is_empty() {
                            if let Some(data) = &self.initial_data {
                                self.conversation_history.push(
                                    chat_completion::ChatCompletionMessage {
                                        role: chat_completion::MessageRole::system,
                                        content: chat_completion::Content::Text(
                                            SYSTEM_PROMPT.to_string(),
                                        ),
                                        name: None,
                                        tool_calls: None,
                                        tool_call_id: None,
                                    },
                                );
                                self.conversation_history.push(
                                    chat_completion::ChatCompletionMessage {
                                        role: chat_completion::MessageRole::user,
                                        content: chat_completion::Content::Text(data.clone()),
                                        name: None,
                                        tool_calls: None,
                                        tool_call_id: None,
                                    },
                                );
                            }
                        }
                        return true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        let rect = self.last_rect.borrow();
                        let visible_height = rect.height.saturating_sub(2) as usize; // Subtract 2 for borders
                        let content_lines: Vec<_> = self.content.lines().collect();
                        let max_scroll = content_lines.len().saturating_sub(visible_height.max(1));
                        self.scroll_offset =
                            std::cmp::min(self.scroll_offset.saturating_add(1), max_scroll);
                        return true;
                    }
                    KeyCode::Char('g') => {
                        // Check if this is the second 'g' press (gg = go to top)
                        let now = std::time::Instant::now();
                        let is_double_press = if let Some(last_time) = self.last_g_press {
                            now.duration_since(last_time).as_millis() < 500
                        } else {
                            false
                        };
                        self.last_g_press = Some(now);

                        if is_double_press {
                            self.scroll_offset = 0;
                            return true;
                        }
                        return false; // First 'g' press, wait for potential second
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        return true;
                    }
                    KeyCode::PageDown => {
                        let rect = self.last_rect.borrow();
                        let page_size = rect.height.saturating_sub(2) as usize;
                        let content_lines: Vec<_> = self.content.lines().collect();
                        let max_scroll = content_lines.len().saturating_sub(page_size);
                        self.scroll_offset =
                            std::cmp::min(self.scroll_offset.saturating_add(page_size), max_scroll);
                        return true;
                    }
                    KeyCode::PageUp => {
                        let rect = self.last_rect.borrow();
                        let page_size = rect.height.saturating_sub(2) as usize;
                        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
                        return true;
                    }
                    KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        let rect = self.last_rect.borrow();
                        let visible_height = rect.height.saturating_sub(2) as usize; // Subtract 2 for borders
                        let content_lines: Vec<_> = self.content.lines().collect();
                        let max_scroll = content_lines.len().saturating_sub(visible_height.max(1));
                        self.scroll_offset = max_scroll;
                        return true;
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log::debug!("Ctrl+r pressed, requesting LLM advice refresh");
                        self.content = "⏳ Loading LLM advice...".to_string();
                        self.scroll_offset = 0;
                        log::debug!("Set advice pane content to loading message");
                        self.refresh_requested = true;
                        return true;
                    }
                    _ => return false,
                }
            }
        }

        if let AppEvent::DataUpdated(_, data) = event {
            self.content = data.clone();
            self.scroll_offset = 0;
            self.conversation_history.clear();
            self.initial_data = Some(data.clone());
            return true;
        }

        false
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn as_advice_pane(&self) -> Option<&AdvicePane> {
        Some(self)
    }

    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePane> {
        Some(self)
    }
}

// Commit Summary Pane Implementation
pub struct CommitSummaryPane {
    visible: bool,
    current_commit: Option<crate::git::CommitInfo>,
    scroll_offset: usize,
    llm_summary: Option<String>,
    llm_client: Option<LlmClient>,
    is_loading_summary: bool,
    pending_summary_sha: Option<String>, // Track which commit we're waiting for a summary for
    llm_shared_state: Option<Arc<LlmSharedState>>,
    loading_state: CommitSummaryLoadingState,
    cache_callback: Option<(String, String)>, // (commit_sha, summary) to cache
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitSummaryLoadingState {
    NoCommit,
    LoadingSummary,
    Loaded,
    Error,
}

impl CommitSummaryPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client: None,
            is_loading_summary: false,
            pending_summary_sha: None,
            llm_shared_state: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
            cache_callback: None,
        }
    }

    pub fn new_with_llm_client(llm_client: Option<LlmClient>) -> Self {
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client,
            is_loading_summary: false,
            pending_summary_sha: None,
            llm_shared_state: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
            cache_callback: None,
        }
    }

    pub fn update_commit(&mut self, commit: Option<crate::git::CommitInfo>) {
        let commit_changed = match (&self.current_commit, &commit) {
            (Some(old), Some(new)) => old.sha != new.sha,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        };

        self.current_commit = commit;

        if commit_changed {
            // Reset state when commit changes
            self.llm_summary = None;
            self.scroll_offset = 0;
            self.is_loading_summary = false;
            self.pending_summary_sha = None;
            self.clear_error();
            self.cache_callback = None;

            // Update loading state based on new commit
            if self.current_commit.is_some() {
                // Since commits from get_commit_history already have files_changed populated,
                // we can immediately show the files and only wait for LLM summary
                self.loading_state = CommitSummaryLoadingState::Loaded;
                // Don't request LLM summary immediately - let the App check cache first
            } else {
                self.loading_state = CommitSummaryLoadingState::NoCommit;
            }
        }
    }

    pub fn set_shared_state(&mut self, llm_shared_state: Arc<LlmSharedState>) {
        self.llm_shared_state = Some(llm_shared_state);
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_state = CommitSummaryLoadingState::Error;
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.set_error("commit_summary".to_string(), error);
        }
        self.is_loading_summary = false;
        self.pending_summary_sha = None;
    }

    pub fn clear_error(&mut self) {
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.clear_error("commit_summary");
        }
    }

    pub fn get_error(&self) -> Option<String> {
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.get_error("commit_summary")
        } else {
            None
        }
    }

    fn request_llm_summary(&mut self) {
        if let Some(_commit) = &self.current_commit {
            // TODO: Implement LLM summary generation using shared state
            self.loading_state = CommitSummaryLoadingState::Loaded;
            self.llm_summary = Some("LLM summary generation not yet implemented with shared state".to_string());
        }
    }

    pub fn poll_llm_summary(&mut self) {
        // This method is now deprecated - use shared state instead
        // The actual summary polling is handled through shared state in the main loop
    }

    /// Set a cached summary directly without generating a new one
    pub fn set_cached_summary(&mut self, commit_sha: &str, summary: String) {
        if let Some(current_commit) = &self.current_commit {
            if current_commit.sha == commit_sha {
                self.llm_summary = Some(summary);
                self.clear_error();
                self.is_loading_summary = false;
                self.pending_summary_sha = None;
                self.loading_state = CommitSummaryLoadingState::Loaded;
            }
        }
    }

    /// Check if we need to request a summary for the current commit
    pub fn needs_summary(&self) -> bool {
        if let Some(_current_commit) = &self.current_commit {
            // Need summary if we don't have one and we're not currently loading
            self.llm_summary.is_none() && !self.is_loading_summary
        } else {
            false
        }
    }

    /// Get the current commit SHA if available
    pub fn get_current_commit_sha(&self) -> Option<String> {
        self.current_commit.as_ref().map(|c| c.sha.clone())
    }

    /// Force generation of a new summary (bypassing cache)
    pub fn force_generate_summary(&mut self) {
        self.llm_summary = None;
        self.clear_error();
        self.request_llm_summary();
    }

    /// Get and clear any pending cache callback
    pub fn take_cache_callback(&mut self) -> Option<(String, String)> {
        self.cache_callback.take()
    }
}

impl Pane for CommitSummaryPane {
    fn title(&self) -> String {
        "Commit Details".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
        };

        let theme = app.get_theme();

        // Handle different loading states
        match self.loading_state {
            CommitSummaryLoadingState::NoCommit => {
                let paragraph = Paragraph::new("No commit selected").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }

            CommitSummaryLoadingState::Error => {
                let error_text = if let Some(error) = self.get_error() {
                    format!("❌ Error loading commit details:\n{}", error)
                } else {
                    "❌ Error loading commit details".to_string()
                };

                let paragraph = Paragraph::new(error_text)
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }
            _ => {} // Continue with normal rendering for LoadingSummary and Loaded states
        }

        if let Some(commit) = &self.current_commit {
            // Validate commit data before rendering
            if commit.sha.is_empty() {
                let paragraph = Paragraph::new("❌ Invalid commit data")
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }

            // Split the area into two sections: file changes and LLM summary
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);

            // Render file changes section
            let mut file_items = Vec::new();

            if commit.files_changed.is_empty() {
                // Show message when no file changes are available
                file_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "ℹ️  No file changes detected",
                    Style::default().fg(theme.secondary_color()),
                )])));
                file_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "   This might be a merge commit or there was an error parsing changes",
                    Style::default().fg(theme.foreground_color()),
                )])));
            } else {
                for (index, file_change) in commit.files_changed.iter().enumerate() {
                    if index < self.scroll_offset {
                        continue;
                    }

                    let visible_height = chunks[0].height.saturating_sub(2) as usize; // Account for borders
                    if file_items.len() >= visible_height {
                        break;
                    }

                    let mut spans = Vec::new();

                    // Status indicator with validation
                    let status_char = match file_change.status {
                        crate::git::FileChangeStatus::Added => "📄 ",
                        crate::git::FileChangeStatus::Modified => "📝 ",
                        crate::git::FileChangeStatus::Deleted => "🗑️  ",
                        crate::git::FileChangeStatus::Renamed => "📋 ",
                    };
                    spans.push(Span::raw(status_char));

                    // File path with length validation
                    let file_path_str = file_change.path.to_string_lossy();
                    let display_path = if file_path_str.len() > 80 {
                        format!("...{}", &file_path_str[file_path_str.len() - 77..])
                    } else {
                        file_path_str.to_string()
                    };

                    spans.push(Span::styled(
                        display_path,
                        Style::default().fg(theme.foreground_color()),
                    ));

                    // Addition/deletion counts with validation
                    if file_change.additions > 0 {
                        let additions_text = if file_change.additions > 9999 {
                            " (+9999+)".to_string()
                        } else {
                            format!(" (+{})", file_change.additions)
                        };
                        spans.push(Span::styled(
                            additions_text,
                            Style::default()
                                .fg(theme.added_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    if file_change.deletions > 0 {
                        let deletions_text = if file_change.deletions > 9999 {
                            " (-9999+)".to_string()
                        } else {
                            format!(" (-{})", file_change.deletions)
                        };
                        spans.push(Span::styled(
                            deletions_text,
                            Style::default()
                                .fg(theme.removed_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    let line = Line::from(spans);
                    file_items.push(ListItem::new(line));
                }
            }

            let file_list = List::new(file_items).block(
                Block::default()
                    .title(format!("Files Changed ({})", commit.files_changed.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );

            f.render_widget(file_list, chunks[0]);

            // Render LLM summary section with enhanced error handling and loading states
            let summary_content = if let Some(summary) = &self.llm_summary {
                summary.clone()
            } else if self.is_loading_summary {
                "⏳ Generating summary...".to_string()
            } else if self.llm_client.is_none() {
                "LLM client not available".to_string()
            } else {
                "📋 Checking cache...".to_string()
            };

            let summary_lines: Vec<Line> = summary_content
                .lines()
                .map(|line| Line::from(line.to_string()))
                .collect();

            let summary_paragraph = Paragraph::new(summary_lines)
                .block(
                    Block::default()
                        .title("LLM Summary")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(summary_paragraph, chunks[1]);
        } else {
            // No commit selected
            let paragraph = Paragraph::new("No commit selected").block(
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
            AppEvent::Key(key) => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(commit) = &self.current_commit {
                            let max_scroll = commit.files_changed.len().saturating_sub(1);
                            self.scroll_offset =
                                std::cmp::min(self.scroll_offset.saturating_add(1), max_scroll);
                        }
                        true
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        true
                    }
                    KeyCode::PageDown => {
                        if let Some(commit) = &self.current_commit {
                            let page_size = 10; // Approximate page size
                            let max_scroll = commit.files_changed.len().saturating_sub(page_size);
                            self.scroll_offset = std::cmp::min(
                                self.scroll_offset.saturating_add(page_size),
                                max_scroll,
                            );
                        }
                        true
                    }
                    KeyCode::PageUp => {
                        let page_size = 10; // Approximate page size
                        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
                        true
                    }
                    KeyCode::Char('g') => {
                        // Go to top
                        self.scroll_offset = 0;
                        true
                    }
                    KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        // Go to bottom
                        if let Some(commit) = &self.current_commit {
                            self.scroll_offset = commit.files_changed.len().saturating_sub(1);
                        }
                        true
                    }
                    _ => false,
                }
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

    fn as_commit_summary_pane(&self) -> Option<&CommitSummaryPane> {
        Some(self)
    }

    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use std::env;

    fn create_test_pane_registry() -> PaneRegistry {
        let mut llm_config = LlmConfig::default();
        if env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = LlmClient::new(llm_config).unwrap();
        let llm_shared_state = Arc::new(LlmSharedState::new());
        PaneRegistry::new(Theme::Dark, llm_client, llm_shared_state)
    }

    #[test]
    fn test_pane_registry_creation() {
        let registry = create_test_pane_registry();
        assert_eq!(registry.panes.len(), 9); // Default panes + advice + commit picker + commit summary
        assert!(registry.get_pane(&PaneId::FileTree).is_some());
        assert!(registry.get_pane(&PaneId::Monitor).is_some());
        assert!(registry.get_pane(&PaneId::Diff).is_some());
        assert!(registry.get_pane(&PaneId::Advice).is_some());
        assert!(registry.get_pane(&PaneId::CommitPicker).is_some());
        assert!(registry.get_pane(&PaneId::CommitSummary).is_some());
    }

    #[test]
    fn test_commit_picker_pane_navigation() {
        let mut pane = CommitPickerPane::new();

        // Test with empty commits
        assert_eq!(pane.current_index, 0);
        pane.navigate_next();
        assert_eq!(pane.current_index, 0);
        pane.navigate_prev();
        assert_eq!(pane.current_index, 0);

        // Add some test commits
        let commits = vec![
            crate::git::CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "First commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-01".to_string(),
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-02".to_string(),
                files_changed: vec![],
            },
        ];

        pane.update_commits(commits);

        // Test navigation
        assert_eq!(pane.current_index, 0);
        pane.navigate_next();
        assert_eq!(pane.current_index, 1);
        pane.navigate_next();
        assert_eq!(pane.current_index, 0); // Should wrap around

        pane.navigate_prev();
        assert_eq!(pane.current_index, 1); // Should wrap around backwards
        pane.navigate_prev();
        assert_eq!(pane.current_index, 0);
    }

    #[test]
    fn test_commit_picker_pane_key_handling() {
        let mut pane = CommitPickerPane::new();

        // Add test commits
        let commits = vec![
            crate::git::CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "First commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-01".to_string(),
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-02".to_string(),
                files_changed: vec![],
            },
        ];

        pane.update_commits(commits);

        // Test j key (next)
        let j_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(pane.handle_event(&j_event));
        assert_eq!(pane.current_index, 1);

        // Test k key (prev)
        let k_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(pane.handle_event(&k_event));
        assert_eq!(pane.current_index, 0);

        // Test g+t combination
        let g_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(pane.handle_event(&g_event));

        // Immediately follow with t
        let t_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(pane.handle_event(&t_event));
        assert_eq!(pane.current_index, 1); // Should navigate next
    }

    #[test]
    fn test_commit_summary_pane_creation() {
        let pane = CommitSummaryPane::new();
        assert!(!pane.visible());
        assert!(pane.current_commit.is_none());
        assert_eq!(pane.scroll_offset, 0);
        assert!(pane.llm_summary.is_none());
    }

    #[test]
    fn test_commit_summary_pane_update_commit() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![crate::git::CommitFileChange {
                path: std::path::PathBuf::from("test.rs"),
                status: crate::git::FileChangeStatus::Modified,
                additions: 5,
                deletions: 2,
            }],
        };

        pane.update_commit(Some(commit.clone()));
        assert!(pane.current_commit.is_some());
        assert_eq!(pane.current_commit.as_ref().unwrap().sha, "abc123");
        assert_eq!(pane.scroll_offset, 0);
        assert!(pane.llm_summary.is_none());
    }

    #[test]
    fn test_commit_summary_pane_scrolling() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: (0..20)
                .map(|i| crate::git::CommitFileChange {
                    path: std::path::PathBuf::from(format!("file{}.rs", i)),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: i,
                    deletions: i / 2,
                })
                .collect(),
        };

        pane.update_commit(Some(commit));

        // Test j key (scroll down)
        let j_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(pane.handle_event(&j_event));
        assert_eq!(pane.scroll_offset, 1);

        // Test k key (scroll up)
        let k_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(pane.handle_event(&k_event));
        assert_eq!(pane.scroll_offset, 0);

        // Test page down
        let page_down_event = AppEvent::Key(KeyEvent::from(KeyCode::PageDown));
        assert!(pane.handle_event(&page_down_event));
        assert_eq!(pane.scroll_offset, 10);

        // Test page up
        let page_up_event = AppEvent::Key(KeyEvent::from(KeyCode::PageUp));
        assert!(pane.handle_event(&page_up_event));
        assert_eq!(pane.scroll_offset, 0);

        // Test go to bottom (Shift+G)
        let bottom_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(pane.handle_event(&bottom_event));
        assert_eq!(pane.scroll_offset, 19);

        // Test go to top (g)
        let top_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(pane.handle_event(&top_event));
        assert_eq!(pane.scroll_offset, 0);
    }

    #[test]
    fn test_commit_summary_pane_llm_summary() {
        let mut pane = CommitSummaryPane::new();

        // Test that initially there's no summary
        assert!(pane.llm_summary.is_none());
        assert!(!pane.is_loading_summary);

        // Test that we can manually set a summary (for testing purposes)
        pane.llm_summary = Some("This is a test summary".to_string());
        assert!(pane.llm_summary.is_some());
        assert_eq!(pane.llm_summary.as_ref().unwrap(), "This is a test summary");
    }

    #[test]
    fn test_commit_summary_pane_with_llm_client() {
        use crate::config::LlmConfig;

        // Create a test LLM client
        let mut llm_config = LlmConfig::default();
        llm_config.api_key = Some("test_key".to_string());
        let llm_client = LlmClient::new(llm_config).ok();

        let pane = CommitSummaryPane::new_with_llm_client(llm_client);

        // Test that the pane has an LLM client
        assert!(pane.llm_client.is_some());
        assert!(pane.llm_summary.is_none());
        assert!(!pane.is_loading_summary);

        // Test that a pane without LLM client works too
        let pane_no_llm = CommitSummaryPane::new_with_llm_client(None);
        assert!(pane_no_llm.llm_client.is_none());
        assert!(pane_no_llm.llm_summary.is_none());
        assert!(!pane_no_llm.is_loading_summary);
    }

    #[test]
    fn test_commit_files_display_immediately() {
        let mut pane = CommitSummaryPane::new();
        
        // Create a commit with file changes (simulating data from get_commit_history)
        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("src/main.rs"),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: 10,
                    deletions: 5,
                },
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    status: crate::git::FileChangeStatus::Added,
                    additions: 20,
                    deletions: 0,
                },
            ],
        };

        // Update the pane with the commit
        pane.update_commit(Some(commit.clone()));

        // Verify that the pane is immediately in Loaded state (not LoadingFiles)
        assert_eq!(pane.loading_state, CommitSummaryLoadingState::Loaded);
        
        // Verify that the commit data is available
        assert!(pane.current_commit.is_some());
        let current_commit = pane.current_commit.as_ref().unwrap();
        assert_eq!(current_commit.files_changed.len(), 2);
        assert_eq!(current_commit.files_changed[0].path, std::path::PathBuf::from("src/main.rs"));
        assert_eq!(current_commit.files_changed[1].path, std::path::PathBuf::from("src/lib.rs"));
        
        // LLM summary should still be None (not loaded yet)
        assert!(pane.llm_summary.is_none());
        
        // But the files should be immediately available for display
        // (This would be verified in the render method, which would show files immediately)
    }

    #[test]
    fn test_commit_summary_pane_race_condition_handling() {
        let mut pane = CommitSummaryPane::new();

        // Create two different commits
        let commit1 = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "First commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };

        let commit2 = crate::git::CommitInfo {
            sha: "def456".to_string(),
            short_sha: "def456".to_string(),
            message: "Second commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-02".to_string(),
            files_changed: vec![],
        };

        // Set first commit
        pane.update_commit(Some(commit1.clone()));
        assert_eq!(pane.current_commit.as_ref().unwrap().sha, "abc123");
        assert!(pane.llm_summary.is_none());

        // Switch to second commit (simulating quick navigation)
        pane.update_commit(Some(commit2.clone()));
        assert_eq!(pane.current_commit.as_ref().unwrap().sha, "def456");
        assert!(pane.llm_summary.is_none());

        // Simulate receiving a stale response for the first commit
        // This should be ignored since we're now on commit2

        // Poll for the response
        pane.poll_llm_summary();

        // The summary should still be None because the response was for a different commit
        assert!(pane.llm_summary.is_none());

        // Now simulate receiving the correct response for commit2 using shared state

        // Poll for the response
        pane.poll_llm_summary();

        // Now the summary should be set
        assert!(pane.llm_summary.is_some());
        assert_eq!(
            pane.llm_summary.as_ref().unwrap(),
            "Summary for second commit"
        );
    }

    #[test]
    fn test_pane_visibility() {
        let registry = create_test_pane_registry();

        let file_tree = registry.get_pane(&PaneId::FileTree).unwrap();
        assert!(file_tree.visible());

        let monitor = registry.get_pane(&PaneId::Monitor).unwrap();
        assert!(!monitor.visible());

        let status_bar = registry.get_pane(&PaneId::StatusBar).unwrap();
        assert!(status_bar.visible());
    }

    #[test]
    fn test_pane_ids() {
        assert_eq!(PaneId::FileTree, PaneId::FileTree);
        assert_ne!(PaneId::FileTree, PaneId::Monitor);
    }

    #[test]
    fn test_advice_pane_scrolling() {
        let mut llm_config = LlmConfig::default();
        if env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = LlmClient::new(llm_config).unwrap();
        let mut advice_pane = AdvicePane::new(Some(llm_client));
        advice_pane.visible = true; // Make the pane visible so it can handle events
        advice_pane.content = (0..100).map(|i| format!("Line {i} with some additional text to make it longer and ensure wrapping occurs")).collect::<Vec<_>>().join("\n");
        let rect = Rect::new(0, 0, 80, 20);
        *advice_pane.last_rect.borrow_mut() = rect;

        // Test that content requires scrolling
        let content_lines: Vec<_> = advice_pane.content.lines().collect();
        let page_size = 18; // height 20 minus 2 for borders
        let max_scroll = content_lines.len().saturating_sub(page_size);
        assert!(max_scroll > 0, "Content should require scrolling");

        // Page down
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::PageDown)));
        assert_eq!(advice_pane.scroll_offset, 18);

        // Page down again
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::PageDown)));
        assert_eq!(advice_pane.scroll_offset, 36);

        // Page up
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::PageUp)));
        assert_eq!(advice_pane.scroll_offset, 18);

        // Scroll to bottom
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::new(
            KeyCode::Char('G'),
            KeyModifiers::SHIFT,
        )));
        assert_eq!(advice_pane.scroll_offset, 82);

        // Page up from bottom
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::PageUp)));
        assert_eq!(advice_pane.scroll_offset, 64);
    }

    // #[test]
    // fn test_advice_pane_jk_navigation() {
    //     let mut llm_config = LlmConfig::default();
    //     if env::var("OPENAI_API_KEY").is_err() {
    //         llm_config.api_key = Some("dummy_key".to_string());
    //     }
    //     let llm_client = LlmClient::new(llm_config).unwrap();
    //     let mut advice_pane = AdvicePane::new(Some(llm_client));
    //     advice_pane.visible = true; // Make the pane visible so it can handle events
    //     advice_pane.content = (0..50).map(|i| format!("Line {i}")).collect::<Vec<_>>().join("\n");
    //     let rect = Rect::new(0, 0, 80, 20); // 20 height, so visible_height = 18 (minus 2 for borders)
    //     *advice_pane.last_rect.borrow_mut() = rect;

    //     // Start at top
    //     assert_eq!(advice_pane.scroll_offset, 0);

    //     // Test j key (scroll down)
    //     let handled = advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::Char('j'))));
    //     assert!(handled, "j key should be handled by advice pane");
    //     assert_eq!(advice_pane.scroll_offset, 1);

    //     // Test multiple j presses
    //     advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::Char('j'))));
    //     advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::Char('j'))));
    //     assert_eq!(advice_pane.scroll_offset, 3);

    //     // Test k key (scroll up)
    //     let handled = advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::Char('k'))));
    //     assert!(handled, "k key should be handled by advice pane");
    //     assert_eq!(advice_pane.scroll_offset, 2);

    //     // Test k key at top (should not go below 0)
    //     advice_pane.scroll_offset = 0;
    //     advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::Char('k'))));
    //     assert_eq!(advice_pane.scroll_offset, 0);

    //     // Test Shift+G (go to bottom)
    //     let handled = advice_pane.handle_event(&AppEvent::Key(KeyEvent::new(
    //         KeyCode::Char('G'),
    //         KeyModifiers::SHIFT,
    //     )));
    //     assert!(handled, "Shift+G should be handled by advice pane");
    //     // With 50 lines of content and visible_height of 18, max_scroll should be 50 - 18 = 32
    //     assert_eq!(advice_pane.scroll_offset, 32);
    // }

    #[test]
    fn test_app_forward_key_to_advice_pane() {
        use crate::ui::{App, Theme};
        use std::sync::Arc;
        
        fn create_test_llm_state() -> Arc<crate::shared_state::LlmSharedState> {
            Arc::new(crate::shared_state::LlmSharedState::new())
        }

        let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());
        
        // Initially advice pane should not be showing
        assert!(!app.is_showing_advice_pane());
        
        // Set advice pane as active
        app.set_advice_pane();
        
        // Verify advice pane is showing
        assert!(app.is_showing_advice_pane());
        
        // We can't directly access pane_registry, but we can test the behavior
        
        // Test that j key is forwarded to advice pane
        let j_key = KeyEvent::from(KeyCode::Char('j'));
        let handled = app.forward_key_to_panes(j_key);
        assert!(handled, "j key should be handled by advice pane when it's visible");
        
        // Test that k key is forwarded to advice pane
        let k_key = KeyEvent::from(KeyCode::Char('k'));
        let handled = app.forward_key_to_panes(k_key);
        assert!(handled, "k key should be handled by advice pane when it's visible");
    }
}
