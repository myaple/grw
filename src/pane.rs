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
use tokio::sync::mpsc;

use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::ui::{ActivePane, App, Theme};

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
    pub fn new(theme: Theme, llm_client: LlmClient) -> Self {
        let mut registry = Self {
            panes: HashMap::new(),
            theme,
        };

        registry.register_default_panes(llm_client);
        registry
    }

    fn register_default_panes(&mut self, llm_client: LlmClient) {
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
        self.register_pane(
            PaneId::CommitSummary,
            Box::new(CommitSummaryPane::new_with_llm_client(Some(llm_client))),
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

        let (pane_title, pane_hotkeys) = match last_active_pane {
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
                    "  /               - Enter input mode",
                    "  Enter           - Submit question",
                    "  Esc             - Exit input mode",
                    "  Ctrl+r          - Refresh LLM advice",
                ],
            ),
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
    llm_tx: mpsc::Sender<Result<String, String>>,
    llm_rx: mpsc::Receiver<Result<String, String>>,
    is_loading: bool,
    input_cursor_position: usize,
    input_scroll_offset: usize,
    initial_data: Option<String>,
    pub refresh_requested: bool,
    last_rect: RefCell<Rect>,
}

impl AdvicePane {
    pub fn new(llm_client: Option<LlmClient>) -> Self {
        let (llm_tx, llm_rx) = mpsc::channel(1);
        Self {
            visible: false,
            content: "⏳ Loading LLM advice...".to_string(),
            scroll_offset: 0,
            input: String::new(),
            input_mode: false,
            conversation_history: Vec::new(),
            llm_client,
            llm_tx,
            llm_rx,
            is_loading: false,
            input_cursor_position: 0,
            input_scroll_offset: 0,
            initial_data: None,
            refresh_requested: false,
            last_rect: RefCell::new(Rect::default()),
        }
    }

    pub fn poll_llm_response(&mut self) {
        if let Ok(result) = self.llm_rx.try_recv() {
            self.is_loading = false;
            match result {
                Ok(response) => {
                    self.content.push_str("\n\n");
                    self.content.push_str(&response);
                    self.conversation_history
                        .push(chat_completion::ChatCompletionMessage {
                            role: chat_completion::MessageRole::assistant,
                            content: chat_completion::Content::Text(response),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        });
                    let content_lines: Vec<_> = self.content.lines().collect();
                    self.scroll_offset = content_lines.len().saturating_sub(1);
                }
                Err(e) => {
                    self.content.push_str("\n\nError: ");
                    self.content.push_str(&e);
                    let content_lines: Vec<_> = self.content.lines().collect();
                    self.scroll_offset = content_lines.len().saturating_sub(1);
                }
            }
        }
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

                        if let Some(llm_client) = self.llm_client.as_ref() {
                            let history = self.conversation_history.clone();
                            let tx = self.llm_tx.clone();
                            let client = llm_client.clone();
                            tokio::spawn(async move {
                                let res = client.get_llm_advice(history).await;
                                let _ = tx.send(res).await;
                            });
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
                        let content_lines: Vec<_> = self.content.lines().collect();
                        let max_scroll = content_lines.len().saturating_sub(1);
                        self.scroll_offset =
                            std::cmp::min(self.scroll_offset.saturating_add(1), max_scroll);
                        return true;
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
                        let content_lines: Vec<_> = self.content.lines().collect();
                        self.scroll_offset = content_lines.len().saturating_sub(1);
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
    llm_tx: mpsc::Sender<Result<(String, String), String>>, // (commit_sha, summary) or error
    llm_rx: mpsc::Receiver<Result<(String, String), String>>,
    is_loading_summary: bool,
    pending_summary_sha: Option<String>, // Track which commit we're waiting for a summary for
    summary_error: Option<String>,
    loading_state: CommitSummaryLoadingState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitSummaryLoadingState {
    NoCommit,
    LoadingFiles,
    LoadingSummary,
    Loaded,
    Error,
}

impl CommitSummaryPane {
    pub fn new() -> Self {
        let (llm_tx, llm_rx) = mpsc::channel(1);
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client: None,
            llm_tx,
            llm_rx,
            is_loading_summary: false,
            pending_summary_sha: None,
            summary_error: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
        }
    }

    pub fn new_with_llm_client(llm_client: Option<LlmClient>) -> Self {
        let (llm_tx, llm_rx) = mpsc::channel(1);
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client,
            llm_tx,
            llm_rx,
            is_loading_summary: false,
            pending_summary_sha: None,
            summary_error: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
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
            self.summary_error = None;

            // Update loading state based on new commit
            if self.current_commit.is_some() {
                self.loading_state = CommitSummaryLoadingState::LoadingFiles;
                self.request_llm_summary();
            } else {
                self.loading_state = CommitSummaryLoadingState::NoCommit;
            }
        }
    }

    pub fn set_error(&mut self, error: String) {
        self.loading_state = CommitSummaryLoadingState::Error;
        self.summary_error = Some(error);
        self.is_loading_summary = false;
        self.pending_summary_sha = None;
    }

    fn request_llm_summary(&mut self) {
        if let Some(commit) = &self.current_commit {
            // Validate commit data before proceeding
            if commit.sha.is_empty() {
                self.set_error("Invalid commit: empty SHA".to_string());
                return;
            }

            if let Some(llm_client) = &self.llm_client {
                self.is_loading_summary = true;
                self.loading_state = CommitSummaryLoadingState::LoadingSummary;
                self.pending_summary_sha = Some(commit.sha.clone());
                self.summary_error = None;

                // Get the full diff content for this commit
                let commit_sha = commit.sha.clone();
                let commit_short_sha = commit.short_sha.clone();
                let commit_message = commit.message.clone();
                let tx = self.llm_tx.clone();
                let client = llm_client.clone();

                tokio::spawn(async move {
                    // Clone commit_sha for use in the blocking task
                    let commit_sha_for_git = commit_sha.clone();

                    // Get the full diff using git show command
                    let diff_result = tokio::task::spawn_blocking(move || {
                        std::process::Command::new("git")
                            .args([
                                "show",
                                "--format=", // Don't show commit message, just the diff
                                "--no-color",
                                &commit_sha_for_git,
                            ])
                            .output()
                    })
                    .await;

                    let full_diff = match diff_result {
                        Ok(Ok(output)) if output.status.success() => {
                            String::from_utf8_lossy(&output.stdout).to_string()
                        }
                        Ok(Ok(output)) => {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            let error_msg = format!("Git show failed: {}", stderr);
                            let _ = tx.send(Err(error_msg)).await;
                            return;
                        }
                        Ok(Err(e)) => {
                            let error_msg = format!("Failed to execute git show: {}", e);
                            let _ = tx.send(Err(error_msg)).await;
                            return;
                        }
                        Err(e) => {
                            let error_msg = format!("Task execution failed: {}", e);
                            let _ = tx.send(Err(error_msg)).await;
                            return;
                        }
                    };

                    // Create a prompt with the full diff content
                    let mut prompt = format!(
                        "Please provide a brief, 2-sentence summary of what this commit changes:\n\n"
                    );
                    prompt.push_str(&format!("Commit: {}\n", commit_short_sha));

                    // Sanitize commit message to prevent prompt injection
                    let sanitized_message = commit_message
                        .replace('\n', " ")
                        .chars()
                        .take(200)
                        .collect::<String>();
                    prompt.push_str(&format!("Message: {}\n\n", sanitized_message));

                    if full_diff.trim().is_empty() {
                        prompt.push_str("No diff content available (this might be a merge commit or have parsing issues).\n");
                    } else {
                        // Limit diff size to prevent overly long prompts (keep first 8000 chars)
                        let truncated_diff = if full_diff.len() > 8000 {
                            let truncated = full_diff.chars().take(8000).collect::<String>();
                            format!("{}\n\n[... diff truncated for brevity ...]", truncated)
                        } else {
                            full_diff
                        };

                        prompt.push_str("Full diff:\n```diff\n");
                        prompt.push_str(&truncated_diff);
                        prompt.push_str("\n```\n");
                    }

                    prompt.push_str("\nFocus on the functional impact and purpose of the changes. Keep it concise and technical.");

                    let history = vec![chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::user,
                        content: chat_completion::Content::Text(prompt),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    }];

                    let res = client.get_llm_advice(history).await;
                    let response = match res {
                        Ok(summary) => {
                            // Validate and sanitize the summary response
                            let sanitized_summary = summary.chars().take(1000).collect::<String>();
                            Ok((commit_sha, sanitized_summary))
                        }
                        Err(e) => {
                            // Provide more specific error messages
                            let error_msg = if e.contains("timeout") {
                                "LLM request timed out. Please try again.".to_string()
                            } else if e.contains("rate limit") {
                                "Rate limit exceeded. Please wait before requesting another summary.".to_string()
                            } else if e.contains("authentication") || e.contains("API key") {
                                "Authentication failed. Please check your API key.".to_string()
                            } else {
                                format!("Failed to generate summary: {}", e)
                            };
                            Err(error_msg)
                        }
                    };
                    let _ = tx.send(response).await;
                });
            } else {
                // No LLM client available - just mark as loaded without setting summary
                self.loading_state = CommitSummaryLoadingState::Loaded;
            }
        }
    }

    pub fn poll_llm_summary(&mut self) {
        if let Ok(result) = self.llm_rx.try_recv() {
            match result {
                Ok((response_commit_sha, summary)) => {
                    // Only use the summary if it's for the currently selected commit
                    if let Some(current_commit) = &self.current_commit {
                        if current_commit.sha == response_commit_sha {
                            // Validate the summary before using it
                            if summary.trim().is_empty() {
                                self.llm_summary = Some("LLM returned empty summary".to_string());
                                self.summary_error = Some("Empty response from LLM".to_string());
                            } else {
                                self.llm_summary = Some(summary);
                                self.summary_error = None;
                            }
                            self.is_loading_summary = false;
                            self.pending_summary_sha = None;
                            self.loading_state = CommitSummaryLoadingState::Loaded;
                        }
                        // If the response is for a different commit, ignore it (stale response)
                    }
                }
                Err(e) => {
                    // Only show error if we're still waiting for a summary for the current commit
                    if let (Some(current_commit), Some(pending_sha)) =
                        (&self.current_commit, &self.pending_summary_sha)
                    {
                        if current_commit.sha == *pending_sha {
                            self.summary_error = Some(e.clone());
                            self.llm_summary = Some(format!("❌ {}", e));
                            self.is_loading_summary = false;
                            self.pending_summary_sha = None;
                            self.loading_state = CommitSummaryLoadingState::Loaded;
                        }
                    }
                }
            }
        }
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
            CommitSummaryLoadingState::LoadingFiles => {
                let paragraph = Paragraph::new("⏳ Loading commit details...").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitSummaryLoadingState::Error => {
                let error_text = if let Some(error) = &self.summary_error {
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

            // Render LLM summary section with enhanced error handling
            let summary_content = if let Some(summary) = &self.llm_summary {
                summary.clone()
            } else if self.is_loading_summary {
                "⏳ Generating summary...".to_string()
            } else if self.llm_client.is_none() {
                "LLM client not available".to_string()
            } else {
                "No summary available".to_string()
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
        PaneRegistry::new(Theme::Dark, llm_client)
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
        let stale_response = Ok(("abc123".to_string(), "Summary for first commit".to_string()));
        let _ = pane.llm_tx.try_send(stale_response);

        // Poll for the response
        pane.poll_llm_summary();

        // The summary should still be None because the response was for a different commit
        assert!(pane.llm_summary.is_none());

        // Now simulate receiving the correct response for commit2
        let correct_response = Ok((
            "def456".to_string(),
            "Summary for second commit".to_string(),
        ));
        let _ = pane.llm_tx.try_send(correct_response);

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
        assert_eq!(advice_pane.scroll_offset, 99);

        // Page up from bottom
        advice_pane.handle_event(&AppEvent::Key(KeyEvent::from(KeyCode::PageUp)));
        assert_eq!(advice_pane.scroll_offset, 81);
    }
}
