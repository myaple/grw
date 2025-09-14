use std::collections::HashMap;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use tokio::sync::mpsc;
use openai_api_rs::v1::chat_completion;

use crate::git::GitRepo;
use crate::llm;
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
    pub fn new(theme: Theme) -> Self {
        let mut registry = Self {
            panes: HashMap::new(),
            theme,
        };

        registry.register_default_panes();
        registry
    }

    fn register_default_panes(&mut self) {
        self.register_pane(PaneId::FileTree, Box::new(FileTreePane::new()));
        self.register_pane(PaneId::Monitor, Box::new(MonitorPane::new()));
        self.register_pane(PaneId::Diff, Box::new(DiffPane::new()));
        self.register_pane(PaneId::SideBySideDiff, Box::new(SideBySideDiffPane::new()));
        self.register_pane(PaneId::Help, Box::new(HelpPane::new()));
        self.register_pane(PaneId::StatusBar, Box::new(StatusBarPane::new()));
        self.register_pane(PaneId::Advice, Box::new(AdvicePane::new()));
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
            log::error!("Error rendering pane {:?}: {}", pane_id, e);
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
            format!("Monitor ⏱️ {} ago", time_str)
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
            format!("{} Hotkeys:", pane_title),
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

        let view_mode_text = match view_mode {
            crate::git::ViewMode::WorkingTree => "💼 Working Tree",
            crate::git::ViewMode::Staged => "📋 Staged Files",
            crate::git::ViewMode::DirtyDirectory => "🗂️ Dirty Directory",
            crate::git::ViewMode::LastCommit => "📜 Last Commit",
        };

        let status_text = format!(
            "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {commit_sha} > {commit_summary} | 📊 {total_files} files (+{total_additions}/-{total_deletions})"
        );

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
    llm_tx: mpsc::Sender<Result<String, String>>,
    llm_rx: mpsc::Receiver<Result<String, String>>,
    is_loading: bool,
    input_cursor_position: usize,
    input_scroll_offset: usize,
    initial_data: Option<String>,
    pub refresh_requested: bool,
}

impl AdvicePane {
    pub fn new() -> Self {
        let (llm_tx, llm_rx) = mpsc::channel(1);
        Self {
            visible: false,
            content: "Press 'Ctrl+l' to open the LLM advice pane. Press '/' to start typing.".to_string(),
            scroll_offset: 0,
            input: String::new(),
            input_mode: false,
            conversation_history: Vec::new(),
            llm_tx,
            llm_rx,
            is_loading: false,
            input_cursor_position: 0,
            input_scroll_offset: 0,
            initial_data: None,
            refresh_requested: false,
        }
    }

    pub fn poll_llm_response(&mut self) {
        if let Ok(result) = self.llm_rx.try_recv() {
            self.is_loading = false;
            match result {
                Ok(response) => {
                    self.content.push_str("\n\n");
                    self.content.push_str(&response);
                    self.conversation_history.push(chat_completion::ChatCompletionMessage {
                        role: chat_completion::MessageRole::assistant,
                        content: chat_completion::Content::Text(response),
                        name: None,
                        tool_calls: None,
                        tool_call_id: None,
                    });
                }
                Err(e) => {
                    self.content.push_str("\n\nError: ");
                    self.content.push_str(&e);
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

        let mut text_lines: Vec<Line> = self.content.lines().map(|l| Line::from(l.to_string())).collect();
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
            f.set_cursor(
                chunks[1].x + (self.input_cursor_position - self.input_scroll_offset) as u16 + 1,
                chunks[1].y + 1,
            );
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        if let AppEvent::Key(key) = event {
            if self.input_mode {
                match key.code {
                    KeyCode::Esc => {
                        self.input_mode = false;
                        return true;
                    }
                    KeyCode::Enter => {
                        let prompt = self.input.drain(..).collect::<String>();
                        self.conversation_history.push(chat_completion::ChatCompletionMessage {
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

                        let history = self.conversation_history.clone();
                        let tx = self.llm_tx.clone();
                        tokio::spawn(async move {
                            let res = llm::get_llm_advice(history).await;
                            let _ = tx.send(res).await;
                        });
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
                        self.input_cursor_position = self.input_cursor_position.saturating_add(1).min(self.input.len());
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
                                self.conversation_history.push(chat_completion::ChatCompletionMessage {
                                    role: chat_completion::MessageRole::system,
                                    content: chat_completion::Content::Text(SYSTEM_PROMPT.to_string()),
                                    name: None,
                                    tool_calls: None,
                                    tool_call_id: None,
                                });
                                self.conversation_history.push(chat_completion::ChatCompletionMessage {
                                    role: chat_completion::MessageRole::user,
                                    content: chat_completion::Content::Text(data.clone()),
                                    name: None,
                                    tool_calls: None,
                                    tool_call_id: None,
                                });
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
                    KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        let content_lines: Vec<_> = self.content.lines().collect();
                        self.scroll_offset = content_lines.len().saturating_sub(1);
                        return true;
                    }
                    KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        log::debug!("Ctrl+r pressed, requesting LLM advice refresh");
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_registry_creation() {
        let registry = PaneRegistry::new(Theme::Dark);
        assert_eq!(registry.panes.len(), 7); // Default panes + advice
        assert!(registry.get_pane(&PaneId::FileTree).is_some());
        assert!(registry.get_pane(&PaneId::Monitor).is_some());
        assert!(registry.get_pane(&PaneId::Diff).is_some());
        assert!(registry.get_pane(&PaneId::Advice).is_some());
    }

    #[test]
    fn test_pane_visibility() {
        let registry = PaneRegistry::new(Theme::Dark);

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
}
