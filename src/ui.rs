use crate::git::{FileDiff, GitRepo, TreeNode, ViewMode};
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn toggle(&mut self) {
        *self = match *self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        };
    }

    pub fn background_color(self) -> Color {
        match self {
            Theme::Dark => Color::Black,
            Theme::Light => Color::White,
        }
    }

    pub fn foreground_color(self) -> Color {
        match self {
            Theme::Dark => Color::White,
            Theme::Light => Color::Black,
        }
    }

    pub fn primary_color(self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    pub fn secondary_color(self) -> Color {
        match self {
            Theme::Dark => Color::Yellow,
            Theme::Light => Color::Yellow,
        }
    }

    pub fn success_color(self) -> Color {
        match self {
            Theme::Dark => Color::Green,
            Theme::Light => Color::DarkGray,
        }
    }

    pub fn error_color(self) -> Color {
        match self {
            Theme::Dark => Color::Red,
            Theme::Light => Color::LightRed,
        }
    }

    pub fn highlight_color(self) -> Color {
        match self {
            Theme::Dark => Color::Blue,
            Theme::Light => Color::LightBlue,
        }
    }

    pub fn border_color(self) -> Color {
        match self {
            Theme::Dark => Color::Gray,
            Theme::Light => Color::DarkGray,
        }
    }

    pub fn muted_color(self) -> Color {
        match self {
            Theme::Dark => Color::DarkGray,
            Theme::Light => Color::Gray,
        }
    }

    pub fn directory_color(self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    pub fn added_color(self) -> Color {
        self.success_color()
    }

    pub fn removed_color(self) -> Color {
        self.error_color()
    }

    pub fn unchanged_color(self) -> Color {
        self.muted_color()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FileBrowserPane {
    FileTree,
    Monitor,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InformationPane {
    Diff,
    SideBySideDiff,
    Help,
    // Add new pane types here in the future
    // Examples:
    // Stats,
    // Blame,
    // History,
    // Search,
}

#[derive(Debug)]
pub struct App {
    files: Vec<FileDiff>,
    current_file_index: usize,
    scroll_offset: usize,
    tree_nodes: Vec<(TreeNode, usize)>,
    current_tree_index: usize,
    file_indices_in_tree: Vec<usize>,
    pub last_g_press: Option<std::time::Instant>,
    pub current_diff_height: usize,
    side_by_side_diff: bool,
    show_diff_panel: bool,
    file_change_timestamps: Vec<std::time::Instant>,
    monitor_output: String,
    monitor_scroll_offset: usize,
    show_monitor_pane: bool,
    monitor_visible_height: usize,
    monitor_command_configured: bool,
    monitor_elapsed_time: Option<std::time::Duration>,
    monitor_has_run: bool,
    current_file_browser_pane: FileBrowserPane,
    current_information_pane: InformationPane,
    theme: Theme,
}

impl App {
    pub fn new_with_config(show_diff_panel: bool, theme: Theme) -> Self {
        Self {
            files: Vec::new(),
            current_file_index: 0,
            scroll_offset: 0,
            tree_nodes: Vec::new(),
            current_tree_index: 0,
            file_indices_in_tree: Vec::new(),
            last_g_press: None,
            current_diff_height: 20,
            side_by_side_diff: false,
            show_diff_panel,
            file_change_timestamps: Vec::new(),
            monitor_output: String::new(),
            monitor_scroll_offset: 0,
            show_monitor_pane: false,
            monitor_visible_height: 10, // Default value
            monitor_command_configured: false,
            monitor_elapsed_time: None,
            monitor_has_run: false,
            current_file_browser_pane: FileBrowserPane::FileTree,
            current_information_pane: InformationPane::Diff,
            theme,
        }
    }

    pub fn update_files(&mut self, files: Vec<FileDiff>) {
        let old_files = std::mem::take(&mut self.files);
        self.files = files;

        // Create a mapping of old file paths to their timestamps
        let old_timestamps: std::collections::HashMap<std::path::PathBuf, std::time::Instant> =
            old_files
                .iter()
                .enumerate()
                .filter_map(|(i, old_file)| {
                    self.file_change_timestamps
                        .get(i)
                        .map(|&ts| (old_file.path.clone(), ts))
                })
                .collect();

        // Build new timestamps, preserving old ones when possible
        let mut new_timestamps = Vec::new();

        for new_file in &self.files {
            if let Some(old_timestamp) = old_timestamps.get(&new_file.path) {
                // File existed before, check if it changed
                if let Some(old_file) = old_files
                    .iter()
                    .find(|old_file| old_file.path == new_file.path)
                {
                    if old_file.line_strings == new_file.line_strings {
                        // File hasn't changed, preserve old timestamp
                        new_timestamps.push(*old_timestamp);
                    } else {
                        // File content changed, update timestamp
                        new_timestamps.push(std::time::Instant::now());
                    }
                } else {
                    // Shouldn't happen, but be safe
                    new_timestamps.push(std::time::Instant::now());
                }
            } else {
                // New file, give it fresh timestamp
                new_timestamps.push(std::time::Instant::now());
            }
        }

        self.file_change_timestamps = new_timestamps;

        if self.current_file_index >= self.files.len() {
            self.current_file_index = 0;
            self.scroll_offset = 0;
        }
    }

    pub fn update_tree(&mut self, tree: &TreeNode) {
        self.tree_nodes = Vec::new();
        self.current_tree_index = 0;
        self.file_indices_in_tree = Vec::new();

        for node in &tree.children {
            self.add_tree_node_recursive(node, 1, &mut Vec::new());
        }

        // Sync current tree index with current file index
        self.sync_tree_index_with_file_index();
    }

    fn add_tree_node_recursive(&mut self, node: &TreeNode, depth: usize, path: &mut Vec<String>) {
        path.push(node.name.clone());

        if node.file_diff.is_some() || !node.children.is_empty() {
            self.tree_nodes.push((node.clone(), depth));

            if node.file_diff.is_some() {
                if let Some(file_index) = self.files.iter().position(|f| f.path == node.path) {
                    self.file_indices_in_tree.push(file_index);
                } else {
                    self.file_indices_in_tree.push(usize::MAX);
                }
            } else {
                self.file_indices_in_tree.push(usize::MAX);
            }
        }

        for child in &node.children {
            self.add_tree_node_recursive(child, depth + 1, path);
        }

        path.pop();
    }

    fn sync_tree_index_with_file_index(&mut self) {
        if let Some(tree_index) = self
            .file_indices_in_tree
            .iter()
            .position(|&idx| idx == self.current_file_index)
        {
            self.current_tree_index = tree_index;
        } else if !self.file_indices_in_tree.is_empty() {
            // Find the first valid file index
            for (i, &file_idx) in self.file_indices_in_tree.iter().enumerate() {
                if file_idx != usize::MAX {
                    self.current_file_index = file_idx;
                    self.current_tree_index = i;
                    break;
                }
            }
        }
    }

    pub fn scroll_down(&mut self, max_lines: usize) {
        if self.current_file_index < self.files.len() {
            let current_file = &self.files[self.current_file_index];
            if self.scroll_offset + max_lines < current_file.line_strings.len() {
                self.scroll_offset += 1;
            }
        }
    }

    pub fn page_down(&mut self, max_lines: usize) {
        if self.current_file_index < self.files.len() {
            let current_file = &self.files[self.current_file_index];
            let total_lines = current_file.line_strings.len();
            if total_lines > max_lines {
                self.scroll_offset = (self.scroll_offset + max_lines).min(total_lines - max_lines);
            }
        }
    }

    pub fn page_up(&mut self, max_lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(max_lines);
    }

    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    pub fn scroll_to_bottom(&mut self, max_lines: usize) {
        if let Some(file) = self.get_current_file() {
            let total_lines = file.line_strings.len();
            if total_lines > max_lines {
                self.scroll_offset = total_lines - max_lines;
            } else {
                self.scroll_offset = 0;
            }
        }
    }

    pub fn scroll_to_top(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn handle_g_press(&mut self) -> bool {
        let now = std::time::Instant::now();
        let is_double_press = if let Some(last_time) = self.last_g_press {
            now.duration_since(last_time).as_millis() < 500
        } else {
            false
        };

        self.last_g_press = Some(now);

        if is_double_press {
            self.scroll_to_top();
            true
        } else {
            false
        }
    }

    pub fn toggle_help(&mut self) {
        if self.current_information_pane == InformationPane::Help {
            // If currently showing help, return to previous diff mode
            self.current_information_pane = if self.side_by_side_diff {
                InformationPane::SideBySideDiff
            } else {
                InformationPane::Diff
            };
        } else {
            // Store current mode and switch to help
            self.current_information_pane = InformationPane::Help;
        }
    }

    pub fn is_showing_help(&self) -> bool {
        self.current_information_pane == InformationPane::Help
    }

    pub fn set_single_pane_diff(&mut self) {
        self.side_by_side_diff = false;
        if !self.is_showing_help() {
            self.current_information_pane = InformationPane::Diff;
        }
    }

    pub fn set_side_by_side_diff(&mut self) {
        self.side_by_side_diff = true;
        if !self.is_showing_help() {
            self.current_information_pane = InformationPane::SideBySideDiff;
        }
    }

    #[allow(dead_code)]
    pub fn is_side_by_side_diff(&self) -> bool {
        self.current_information_pane == InformationPane::SideBySideDiff
    }

    pub fn toggle_diff_panel(&mut self) {
        self.show_diff_panel = !self.show_diff_panel;
    }

    pub fn is_showing_diff_panel(&self) -> bool {
        self.show_diff_panel
    }

    pub fn is_file_recently_changed(&self, file_index: usize) -> bool {
        if let Some(timestamp) = self.file_change_timestamps.get(file_index) {
            timestamp.elapsed().as_secs() < 3
        } else {
            false
        }
    }

    pub fn next_file(&mut self) {
        if !self.files.is_empty() {
            // Find the next file in the tree that has a valid file index
            let start_tree_index = self.current_tree_index;
            let mut next_tree_index = (self.current_tree_index + 1) % self.tree_nodes.len();

            // Look for the next tree node that represents a file
            while next_tree_index != start_tree_index {
                if let Some(&file_idx) = self.file_indices_in_tree.get(next_tree_index)
                    && file_idx != usize::MAX
                {
                    self.current_file_index = file_idx;
                    self.current_tree_index = next_tree_index;
                    self.scroll_offset = 0;
                    return;
                }
                next_tree_index = (next_tree_index + 1) % self.tree_nodes.len();
            }

            // If we couldn't find another file, just cycle through files directly
            self.current_file_index = (self.current_file_index + 1) % self.files.len();
            self.sync_tree_index_with_file_index();
            self.scroll_offset = 0;
        }
    }

    pub fn prev_file(&mut self) {
        if !self.files.is_empty() {
            // Find the previous file in the tree that has a valid file index
            let start_tree_index = self.current_tree_index;
            let mut prev_tree_index = if self.current_tree_index == 0 {
                self.tree_nodes.len() - 1
            } else {
                self.current_tree_index - 1
            };

            // Look for the previous tree node that represents a file
            while prev_tree_index != start_tree_index {
                if let Some(&file_idx) = self.file_indices_in_tree.get(prev_tree_index)
                    && file_idx != usize::MAX
                {
                    self.current_file_index = file_idx;
                    self.current_tree_index = prev_tree_index;
                    self.scroll_offset = 0;
                    return;
                }
                prev_tree_index = if prev_tree_index == 0 {
                    self.tree_nodes.len() - 1
                } else {
                    prev_tree_index - 1
                };
            }

            // If we couldn't find another file, just cycle through files directly
            self.current_file_index = if self.current_file_index == 0 {
                self.files.len() - 1
            } else {
                self.current_file_index - 1
            };
            self.sync_tree_index_with_file_index();
            self.scroll_offset = 0;
        }
    }

    pub fn get_current_file(&self) -> Option<&FileDiff> {
        self.files.get(self.current_file_index)
    }

    pub fn update_monitor_output(&mut self, output: String) {
        self.monitor_output = output;
        // Don't reset scroll offset - preserve user's current scroll position
    }

    pub fn scroll_monitor_down(&mut self) {
        let lines: Vec<&str> = self.monitor_output.lines().collect();
        if !lines.is_empty() {
            // Only scroll if there's more content below the current view
            let max_scroll = lines.len().saturating_sub(self.monitor_visible_height);
            if self.monitor_scroll_offset < max_scroll {
                self.monitor_scroll_offset += 1;
            }
        }
    }

    pub fn scroll_monitor_up(&mut self) {
        if self.monitor_scroll_offset > 0 {
            self.monitor_scroll_offset -= 1;
        }
    }

    pub fn toggle_monitor_pane(&mut self) {
        self.show_monitor_pane = !self.show_monitor_pane;
        if self.show_monitor_pane {
            self.current_file_browser_pane = FileBrowserPane::Monitor;
        } else {
            self.current_file_browser_pane = FileBrowserPane::FileTree;
        }
    }

    pub fn is_showing_monitor_pane(&self) -> bool {
        self.show_monitor_pane
    }

    pub fn set_monitor_visible_height(&mut self, height: usize) {
        self.monitor_visible_height = height;
    }

    pub fn set_monitor_command_configured(&mut self, configured: bool) {
        self.monitor_command_configured = configured;
    }

    pub fn update_monitor_timing(&mut self, elapsed: Option<std::time::Duration>, has_run: bool) {
        self.monitor_elapsed_time = elapsed;
        self.monitor_has_run = has_run;
    }

    fn format_elapsed_time(&self, elapsed: std::time::Duration) -> String {
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{}s", secs)
        } else if secs < 3600 {
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            format!("{}m{}s", mins, remaining_secs)
        } else {
            let hours = secs / 3600;
            let remaining_mins = (secs % 3600) / 60;
            format!("{}h{}m", hours, remaining_mins)
        }
    }

    pub fn get_theme(&self) -> Theme {
        self.theme
    }

    pub fn toggle_theme(&mut self) {
        self.theme.toggle();
    }
}

#[allow(clippy::extra_unused_type_parameters)]
pub fn render<B: Backend>(f: &mut Frame, app: &App, git_repo: &GitRepo) {
    let size = f.area();

    // Allow header to wrap to multiple lines (up to 3 lines)
    let header_constraints = if size.width > 120 {
        // Wide screens: try to fit on one line
        [Constraint::Length(1), Constraint::Min(0)]
    } else {
        // Narrow screens: allow up to 3 lines for header
        [Constraint::Max(3), Constraint::Min(0)]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(header_constraints)
        .split(size);

    render_status_bar(f, app, git_repo, chunks[0]);

    // Handle the information pane (right side)
    if app.is_showing_diff_panel() {
        let bottom_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
            .split(chunks[1]);

        // Render file browser pane (left side)
        render_file_browser_pane(f, app, bottom_chunks[0]);

        // Render information pane (right side)
        let diff_height = bottom_chunks[1].height.saturating_sub(2) as usize;
        render_information_pane(f, app, bottom_chunks[1], diff_height);
    } else {
        // When diff panel is hidden, file browser takes full width
        // But if help is showing, it takes over the full content area
        if app.is_showing_help() {
            render_help_view(f, app, chunks[1]);
        } else {
            render_file_browser_pane(f, app, chunks[1]);
        }
    }
}

fn render_file_browser_pane(f: &mut Frame, app: &App, area: Rect) {
    // If help is showing and diff panel is hidden, help takes over the full area
    if app.is_showing_help() && !app.is_showing_diff_panel() {
        render_help_view(f, app, area);
        return;
    }

    match app.current_file_browser_pane {
        FileBrowserPane::FileTree => {
            if app.is_showing_monitor_pane() {
                // Split the file tree area into tree and monitor sections
                let tree_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Percentage(50), // File tree (top half)
                        Constraint::Percentage(50), // Monitor pane (bottom half)
                    ])
                    .split(area);

                // Render file tree in top half
                render_file_tree_content(f, app, tree_chunks[0]);

                // Render monitor pane in bottom half
                render_monitor_pane(f, app, tree_chunks[1]);
            } else {
                // Monitor pane is hidden, file tree takes full area
                render_file_tree_content(f, app, area);
            }
        }
        FileBrowserPane::Monitor => {
            // When in monitor pane mode, show file tree in top 50% and monitor in bottom 50%
            let tree_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Percentage(50), // File tree (top half)
                    Constraint::Percentage(50), // Monitor pane (bottom half)
                ])
                .split(area);

            // Render file tree in top half
            render_file_tree_content(f, app, tree_chunks[0]);

            // Render monitor pane in bottom half
            render_monitor_pane(f, app, tree_chunks[1]);
        }
    }
}

trait InformationPaneRenderer {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, max_lines: usize);
}

struct DiffRenderer;
struct SideBySideDiffRenderer;
struct HelpRenderer;

impl InformationPaneRenderer for DiffRenderer {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
        render_diff_view(f, app, area, max_lines);
    }
}

impl InformationPaneRenderer for SideBySideDiffRenderer {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
        render_side_by_side_diff_view(f, app, area, max_lines);
    }
}

impl InformationPaneRenderer for HelpRenderer {
    fn render(&self, f: &mut Frame, app: &App, area: Rect, _max_lines: usize) {
        render_help_view(f, app, area);
    }
}

fn render_information_pane(f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
    let renderer: Box<dyn InformationPaneRenderer> = match app.current_information_pane {
        InformationPane::Diff => Box::new(DiffRenderer),
        InformationPane::SideBySideDiff => Box::new(SideBySideDiffRenderer),
        InformationPane::Help => Box::new(HelpRenderer),
    };

    renderer.render(f, app, area, max_lines);
}

fn render_file_tree_content(f: &mut Frame, app: &App, area: Rect) {
    let theme = app.get_theme();
    let tree_items: Vec<ListItem> = app
        .tree_nodes
        .iter()
        .enumerate()
        .map(|(index, (node, depth))| {
            let indent = "  ".repeat(*depth);
            let name_spans = if node.is_dir {
                vec![Span::raw(format!("{}📁 {}", indent, node.name))]
            } else {
                let mut spans = Vec::new();

                // Add arrow for current file selection
                if index == app.current_tree_index {
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
                if let Some(file_idx) = app.files.iter().position(|f| f.path == diff.path) {
                    if file_idx < app.file_change_timestamps.len()
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
                .title("Changed Files")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color())),
        )
        .highlight_style(
            Style::default()
                .fg(theme.secondary_color())
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(file_list, area);
}

fn render_status_bar(f: &mut Frame, app: &App, git_repo: &GitRepo, area: Rect) {
    let theme = app.get_theme();
    let repo_name = git_repo.get_repo_name();
    let branch = git_repo.get_current_branch();
    let (commit_sha, commit_summary) = git_repo.get_last_commit_info();
    let (total_files, total_additions, total_deletions) = git_repo.get_total_stats();
    let view_mode = git_repo.get_current_view_mode();

    // Get view mode display text
    let view_mode_text = match view_mode {
        ViewMode::WorkingTree => "💼 Working Tree",
        ViewMode::Staged => "📋 Staged Files",
        ViewMode::DirtyDirectory => "🗂️ Dirty Directory",
        ViewMode::LastCommit => "📜 Last Commit",
    };

    // Build the complete status text
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
}

fn render_side_by_side_diff_view(f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
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
            if i < app.scroll_offset {
                continue;
            }

            if line_count >= max_lines {
                break;
            }

            let (left_content, right_content) = if let Some(stripped) = line.strip_prefix('+') {
                // Addition: empty on left, content on right
                ("".to_string(), stripped.to_string())
            } else if let Some(stripped) = line.strip_prefix('-') {
                // Deletion: content on left, empty on right
                (stripped.to_string(), "".to_string())
            } else if let Some(stripped) = line.strip_prefix(' ') {
                // Unchanged: same content on both sides
                let content = stripped.to_string();
                (content.clone(), content)
            } else {
                // Header/context line: same content on both sides
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

        let left_text = Text::from(left_lines);
        let right_text = Text::from(right_lines);

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
                .title("Side-by-side Diff")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color())),
        );
        f.render_widget(paragraph, area);
    }
}

fn render_diff_view(f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
    let theme = app.get_theme();
    if let Some(file) = app.get_current_file() {
        let file_path = file.path.to_string_lossy();
        let title = format!("Diff: {file_path}");

        let mut lines = Vec::new();

        for (i, line) in file.line_strings.iter().enumerate() {
            if i < app.scroll_offset {
                continue;
            }

            if lines.len() >= max_lines {
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

        let text = Text::from(lines);
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
                .title("Diff")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color())),
        );
        f.render_widget(paragraph, area);
    }
}

fn render_help_view(f: &mut Frame, app: &App, area: Rect) {
    let theme = app.get_theme();
    let help_text = vec![
        Line::from(Span::styled(
            "Git Repository Watcher - Help",
            Style::default()
                .fg(theme.secondary_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation:",
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  Tab           - Next file"),
        Line::from("  Shift+Tab     - Previous file"),
        Line::from("  g t           - Next file (same as Tab)"),
        Line::from("  g T           - Previous file (same as Shift+Tab)"),
        Line::from(""),
        Line::from(Span::styled(
            "Scrolling:",
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  j / Down      - Scroll down one line"),
        Line::from("  k / Up        - Scroll up one line"),
        Line::from("  Ctrl+e        - Scroll down one line"),
        Line::from("  Ctrl+y        - Scroll up one line"),
        Line::from("  PageDown      - Scroll down one page"),
        Line::from("  PageUp        - Scroll up one page"),
        Line::from("  g g           - Go to top of diff"),
        Line::from("  Shift+G       - Go to bottom of diff"),
        Line::from(""),
        Line::from(Span::styled(
            "Other:",
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ?             - Show/hide this help page"),
        Line::from("  Esc           - Exit help page"),
        Line::from("  Ctrl+S        - Switch to side-by-side diff view"),
        Line::from("  Ctrl+D        - Switch to single-pane diff view"),
        Line::from("  Ctrl+H        - Toggle diff panel visibility"),
        Line::from("  Ctrl+O        - Toggle monitor pane visibility"),
        Line::from("  Ctrl+T        - Toggle light/dark theme"),
        Line::from("  Alt+j         - Scroll monitor pane down"),
        Line::from("  Alt+k         - Scroll monitor pane up"),
        Line::from("  q / Ctrl+C    - Quit application"),
        Line::from(""),
        Line::from(Span::styled(
            "Theme:",
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  The application supports light and dark themes."),
        Line::from("  Use Ctrl+T to toggle between themes at runtime."),
        Line::from("  Theme can also be set via --theme CLI flag or config file."),
        Line::from(""),
        Line::from("Press ? or Esc to return to diff view"),
    ];

    let text = Text::from(help_text);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title("Help")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color())),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn render_monitor_pane(f: &mut Frame, app: &App, area: Rect) {
    let theme = app.get_theme();
    let monitor_lines: Vec<_> = app
        .monitor_output
        .lines()
        .skip(app.monitor_scroll_offset)
        .collect();
    let visible_lines = area.height.saturating_sub(2) as usize; // Account for borders

    // Take only the lines that will fit in the visible area
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

    let title = if !app.monitor_command_configured {
        "Monitor (no command configured)".to_string()
    } else if !app.monitor_has_run {
        "Monitor ⏳ loading...".to_string()
    } else if let Some(elapsed) = app.monitor_elapsed_time {
        let time_str = app.format_elapsed_time(elapsed);
        format!("Monitor ⏱️ {} ago", time_str)
    } else {
        "Monitor Output".to_string()
    };

    let text = Text::from(display_lines);
    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color())),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new_with_config(true, Theme::Dark);
        assert_eq!(app.files.len(), 0);
        assert_eq!(app.current_file_index, 0);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.current_diff_height, 20);
        assert!(!app.is_showing_help());
        assert!(app.show_diff_panel);
        assert_eq!(app.current_file_browser_pane, FileBrowserPane::FileTree);
        assert_eq!(app.current_information_pane, InformationPane::Diff);
        assert!(!app.monitor_command_configured);
        assert!(app.monitor_elapsed_time.is_none());
        assert!(!app.monitor_has_run);
        assert_eq!(app.get_theme(), Theme::Dark);
    }

    #[test]
    fn test_app_creation_no_diff() {
        let app = App::new_with_config(false, Theme::Dark);
        assert!(!app.show_diff_panel);
    }

    #[test]
    fn test_scroll_up() {
        let mut app = App::new_with_config(true, Theme::Dark);
        app.scroll_offset = 5;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 4);
    }

    #[test]
    fn test_scroll_up_at_zero() {
        let mut app = App::new_with_config(true, Theme::Dark);
        app.scroll_offset = 0;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_page_up() {
        let mut app = App::new_with_config(true, Theme::Dark);
        app.scroll_offset = 25;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 15);
    }

    #[test]
    fn test_page_up_underflow() {
        let mut app = App::new_with_config(true, Theme::Dark);
        app.scroll_offset = 5;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_to_top() {
        let mut app = App::new_with_config(true, Theme::Dark);
        app.scroll_offset = 100;
        app.scroll_to_top();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_toggle_help() {
        let mut app = App::new_with_config(true, Theme::Dark);
        assert!(!app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Diff);

        app.toggle_help();
        assert!(app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Help);

        app.toggle_help();
        assert!(!app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Diff);
    }

    #[test]
    fn test_toggle_diff_panel() {
        let mut app = App::new_with_config(true, Theme::Dark);
        assert!(app.show_diff_panel);
        app.toggle_diff_panel();
        assert!(!app.show_diff_panel);
        app.toggle_diff_panel();
        assert!(app.show_diff_panel);
    }

    #[test]
    fn test_monitor_output_update() {
        let mut app = App::new_with_config(true, Theme::Dark);
        assert_eq!(app.monitor_output, "");
        assert_eq!(app.monitor_scroll_offset, 0);

        // Set scroll offset to test that it's preserved
        app.monitor_scroll_offset = 5;

        app.update_monitor_output("test output".to_string());
        assert_eq!(app.monitor_output, "test output");
        assert_eq!(app.monitor_scroll_offset, 5); // Should preserve scroll offset
    }

    #[test]
    fn test_monitor_scroll() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Set a reasonable visible height for testing
        app.monitor_visible_height = 3;

        // Create a long output with multiple lines
        let long_output = "Line 1\nLine 2\nLine 3\nLine 4\nLine 5";
        app.update_monitor_output(long_output.to_string());

        // Test scrolling down
        app.scroll_monitor_down();
        assert_eq!(app.monitor_scroll_offset, 1);

        app.scroll_monitor_down();
        assert_eq!(app.monitor_scroll_offset, 2);

        // Try to scroll past content - should stop at max scroll (5 lines - 3 visible = 2 max scroll)
        app.scroll_monitor_down();
        assert_eq!(app.monitor_scroll_offset, 2); // Should not increase beyond max

        // Test scrolling up
        app.scroll_monitor_up();
        assert_eq!(app.monitor_scroll_offset, 1);

        app.scroll_monitor_up();
        assert_eq!(app.monitor_scroll_offset, 0);

        // Test scrolling up when already at top
        app.scroll_monitor_up();
        assert_eq!(app.monitor_scroll_offset, 0);
    }

    #[test]
    fn test_toggle_monitor_pane() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially monitor pane should be hidden
        assert!(!app.is_showing_monitor_pane());
        assert_eq!(app.current_file_browser_pane, FileBrowserPane::FileTree);

        // Toggle to show monitor pane
        app.toggle_monitor_pane();
        assert!(app.is_showing_monitor_pane());
        assert_eq!(app.current_file_browser_pane, FileBrowserPane::Monitor);

        // Toggle back to hide monitor pane
        app.toggle_monitor_pane();
        assert!(!app.is_showing_monitor_pane());
        assert_eq!(app.current_file_browser_pane, FileBrowserPane::FileTree);
    }

    #[test]
    fn test_monitor_command_configured() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially no command configured
        assert!(!app.monitor_command_configured);

        // Set command as configured
        app.set_monitor_command_configured(true);
        assert!(app.monitor_command_configured);

        // Set command as not configured
        app.set_monitor_command_configured(false);
        assert!(!app.monitor_command_configured);
    }

    #[test]
    fn test_monitor_timing_update() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially no timing info
        assert!(app.monitor_elapsed_time.is_none());
        assert!(!app.monitor_has_run);

        // Update timing info
        let duration = std::time::Duration::from_secs(65);
        app.update_monitor_timing(Some(duration), true);

        assert_eq!(app.monitor_elapsed_time, Some(duration));
        assert!(app.monitor_has_run);
    }

    #[test]
    fn test_format_elapsed_time() {
        let app = App::new_with_config(true, Theme::Dark);

        // Test seconds
        let secs = std::time::Duration::from_secs(45);
        assert_eq!(app.format_elapsed_time(secs), "45s");

        // Test minutes and seconds
        let mins_secs = std::time::Duration::from_secs(125);
        assert_eq!(app.format_elapsed_time(mins_secs), "2m5s");

        // Test hours and minutes
        let hours_mins = std::time::Duration::from_secs(3665);
        assert_eq!(app.format_elapsed_time(hours_mins), "1h1m");
    }

    #[test]
    fn test_diff_mode_switching() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially in single-pane diff mode
        assert_eq!(app.current_information_pane, InformationPane::Diff);
        assert!(!app.is_side_by_side_diff());

        // Switch to side-by-side mode
        app.set_side_by_side_diff();
        assert_eq!(
            app.current_information_pane,
            InformationPane::SideBySideDiff
        );
        assert!(app.is_side_by_side_diff());

        // Switch back to single-pane mode
        app.set_single_pane_diff();
        assert_eq!(app.current_information_pane, InformationPane::Diff);
        assert!(!app.is_side_by_side_diff());
    }

    #[test]
    fn test_help_preserves_diff_mode() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Set to side-by-side mode
        app.set_side_by_side_diff();
        assert_eq!(
            app.current_information_pane,
            InformationPane::SideBySideDiff
        );

        // Show help
        app.toggle_help();
        assert_eq!(app.current_information_pane, InformationPane::Help);

        // Hide help - should return to side-by-side mode
        app.toggle_help();
        assert_eq!(
            app.current_information_pane,
            InformationPane::SideBySideDiff
        );
    }

    #[test]
    fn test_help_movement_when_diff_panel_hidden() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially showing diff panel
        assert!(app.is_showing_diff_panel());
        assert!(!app.is_showing_help());

        // Hide diff panel
        app.toggle_diff_panel();
        assert!(!app.is_showing_diff_panel());

        // Show help - should work even when diff panel is hidden
        app.toggle_help();
        assert!(app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Help);

        // Hide help
        app.toggle_help();
        assert!(!app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Diff);
    }

    #[test]
    fn test_theme_toggle() {
        let mut app = App::new_with_config(true, Theme::Dark);

        // Initially dark theme
        assert_eq!(app.get_theme(), Theme::Dark);

        // Toggle to light theme
        app.toggle_theme();
        assert_eq!(app.get_theme(), Theme::Light);

        // Toggle back to dark theme
        app.toggle_theme();
        assert_eq!(app.get_theme(), Theme::Dark);
    }

    #[test]
    fn test_theme_colors() {
        // Test dark theme colors
        let dark_theme = Theme::Dark;
        assert_eq!(dark_theme.background_color(), Color::Black);
        assert_eq!(dark_theme.foreground_color(), Color::White);
        assert_eq!(dark_theme.added_color(), Color::Green);
        assert_eq!(dark_theme.removed_color(), Color::Red);

        // Test light theme colors
        let light_theme = Theme::Light;
        assert_eq!(light_theme.background_color(), Color::White);
        assert_eq!(light_theme.foreground_color(), Color::Black);
        assert_eq!(light_theme.added_color(), Color::DarkGray);
        assert_eq!(light_theme.removed_color(), Color::LightRed);
    }
}
