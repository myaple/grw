use crate::git::{FileDiff, GitRepo, TreeNode};
use crate::llm::LlmClient;
use crate::pane::{PaneId, PaneRegistry};
use crossterm::event::KeyEvent;
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
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

    pub fn directory_color(self) -> Color {
        match self {
            Theme::Dark => Color::Cyan,
            Theme::Light => Color::Blue,
        }
    }

    pub fn added_color(self) -> Color {
        Color::Green
    }

    pub fn removed_color(self) -> Color {
        self.error_color()
    }

    pub fn unchanged_color(self) -> Color {
        self.foreground_color()
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
    Advice,
    // Add new pane types here in the future
    // Examples:
    // Stats,
    // Blame,
    // History,
    // Search,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ActivePane {
    #[default]
    FileTree,
    Monitor,
    Diff,
    SideBySideDiff,
    Advice,
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
    show_changed_files_pane: bool,
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
    pane_registry: PaneRegistry,
    llm_advice: String,
    last_active_pane: ActivePane,
}

impl App {
    pub fn new_with_config(
        show_diff_panel: bool,
        show_changed_files_pane: bool,
        theme: Theme,
        llm_client: Option<LlmClient>,
    ) -> Self {
        let pane_registry = if let Some(llm_client) = llm_client {
            PaneRegistry::new(theme, llm_client)
        } else {
            // Provide a dummy or default LlmClient for the registry when none is available.
            // This depends on how PaneRegistry and AdvicePane are structured.
            // For now, let's assume AdvicePane can handle a missing LlmClient.
            let mut dummy_llm_config = crate::config::LlmConfig::default();
            if std::env::var("OPENAI_API_KEY").is_err() {
                dummy_llm_config.api_key = Some("dummy_key".to_string());
            }
            let dummy_llm_client = LlmClient::new(dummy_llm_config).unwrap();
            PaneRegistry::new(theme, dummy_llm_client)
        };

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
            show_changed_files_pane,
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
            pane_registry,
            llm_advice: String::new(),
            last_active_pane: ActivePane::default(),
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
        let help_pane_visible = if let Some(help_pane) = self.pane_registry.get_pane(&PaneId::Help)
        {
            help_pane.visible()
        } else {
            false
        };

        if help_pane_visible {
            // Hide help pane
            self.pane_registry
                .with_pane_mut(&PaneId::Help, |help_pane| {
                    help_pane.set_visible(false);
                });
            // Hide help pane and restore the last active pane
            match self.last_active_pane {
                ActivePane::FileTree => {
                    // This case implies the information pane was not shown.
                    // We can't restore this state perfectly without more info,
                    // so we'll default to the standard diff view.
                    self.set_single_pane_diff();
                    self.current_information_pane = InformationPane::Diff;
                }
                ActivePane::Monitor => {
                    // Same as above, monitor is on the left. Default to diff view on right.
                    self.set_single_pane_diff();
                    self.current_information_pane = InformationPane::Diff;
                }
                ActivePane::Diff => {
                    self.set_single_pane_diff();
                    self.current_information_pane = InformationPane::Diff;
                }
                ActivePane::SideBySideDiff => {
                    self.set_side_by_side_diff();
                    self.current_information_pane = InformationPane::SideBySideDiff;
                }
                ActivePane::Advice => {
                    self.set_advice_pane();
                    self.current_information_pane = InformationPane::Advice;
                }
            }
        } else {
            // Determine which pane is active before showing help
            if self.is_showing_advice_pane() {
                self.last_active_pane = ActivePane::Advice;
            } else if self
                .pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .is_some_and(|p| p.visible())
            {
                self.last_active_pane = ActivePane::SideBySideDiff;
            } else if self
                .pane_registry
                .get_pane(&PaneId::Diff)
                .is_some_and(|p| p.visible())
            {
                self.last_active_pane = ActivePane::Diff;
            } else if self.is_showing_monitor_pane() {
                self.last_active_pane = ActivePane::Monitor;
            } else {
                self.last_active_pane = ActivePane::FileTree;
            }

            // Show help pane
            self.pane_registry
                .with_pane_mut(&PaneId::Help, |help_pane| {
                    help_pane.set_visible(true);
                });
            // Hide all other information panes
            self.pane_registry
                .with_pane_mut(&PaneId::Diff, |diff_pane| {
                    diff_pane.set_visible(false);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::SideBySideDiff, |diff_pane| {
                    diff_pane.set_visible(false);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::Advice, |advice_pane| {
                    advice_pane.set_visible(false);
                });
            // Update the legacy field for backward compatibility
            self.current_information_pane = InformationPane::Help;
        }
    }

    pub fn is_showing_help(&self) -> bool {
        if let Some(help_pane) = self.pane_registry.get_pane(&PaneId::Help) {
            help_pane.visible()
        } else {
            false
        }
    }

    pub fn set_single_pane_diff(&mut self) {
        self.side_by_side_diff = false;
        if !self.is_showing_help() {
            self.pane_registry
                .with_pane_mut(&PaneId::Diff, |diff_pane| {
                    diff_pane.set_visible(true);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::SideBySideDiff, |diff_pane| {
                    diff_pane.set_visible(false);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::Advice, |advice_pane| {
                    advice_pane.set_visible(false);
                });
        }
    }

    pub fn set_side_by_side_diff(&mut self) {
        self.side_by_side_diff = true;
        if !self.is_showing_help() {
            self.pane_registry
                .with_pane_mut(&PaneId::SideBySideDiff, |diff_pane| {
                    diff_pane.set_visible(true);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::Diff, |diff_pane| {
                    diff_pane.set_visible(false);
                });
            self.pane_registry
                .with_pane_mut(&PaneId::Advice, |advice_pane| {
                    advice_pane.set_visible(false);
                });
        }
    }

    #[allow(dead_code)]
    pub fn is_side_by_side_diff(&self) -> bool {
        if let Some(diff_pane) = self.pane_registry.get_pane(&PaneId::SideBySideDiff) {
            diff_pane.visible()
        } else {
            false
        }
    }

    pub fn toggle_diff_panel(&mut self) {
        self.show_diff_panel = !self.show_diff_panel;
    }

    pub fn is_showing_diff_panel(&self) -> bool {
        self.show_diff_panel
    }

    pub fn toggle_changed_files_pane(&mut self) {
        self.show_changed_files_pane = !self.show_changed_files_pane;
    }

    pub fn is_showing_changed_files_pane(&self) -> bool {
        self.show_changed_files_pane
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
        self.monitor_output = output.clone();
        // Update the pane registry as well
        self.pane_registry
            .with_pane_mut(&PaneId::Monitor, |monitor_pane| {
                let _ = monitor_pane.handle_event(&crate::pane::AppEvent::DataUpdated((), output));
            });
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
        self.pane_registry
            .with_pane_mut(&PaneId::Monitor, |monitor_pane| {
                monitor_pane.set_visible(self.show_monitor_pane);
            });
        if self.show_monitor_pane {
            self.current_file_browser_pane = FileBrowserPane::Monitor;
        } else {
            self.current_file_browser_pane = FileBrowserPane::FileTree;
        }
    }

    pub fn is_showing_monitor_pane(&self) -> bool {
        self.show_monitor_pane
    }

    pub fn is_showing_advice_pane(&self) -> bool {
        matches!(self.current_information_pane, InformationPane::Advice)
    }

    pub fn forward_key_to_panes(&mut self, key: KeyEvent) -> bool {
        let mut handled = false;

        // Forward to advice pane if it's visible
        if self.is_showing_advice_pane()
            && let Some(pane_handled) = self.pane_registry.with_pane_mut(&PaneId::Advice, |pane| {
                pane.handle_event(&crate::pane::AppEvent::Key(key))
            })
        {
            handled |= pane_handled;
        }

        handled
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

    pub fn format_elapsed_time(&self, elapsed: std::time::Duration) -> String {
        let secs = elapsed.as_secs();
        if secs < 60 {
            format!("{secs}s")
        } else if secs < 3600 {
            let mins = secs / 60;
            let remaining_secs = secs % 60;
            format!("{mins}m{remaining_secs}s")
        } else {
            let hours = secs / 3600;
            let remaining_mins = (secs % 3600) / 60;
            format!("{hours}h{remaining_mins}m")
        }
    }

    pub fn get_theme(&self) -> Theme {
        self.theme
    }

    pub fn toggle_theme(&mut self) {
        self.theme.toggle();
        self.pane_registry.set_theme(self.theme);
    }

    pub fn set_advice_pane(&mut self) {
        self.pane_registry
            .with_pane_mut(&PaneId::Advice, |p| p.set_visible(true));
        // Hide other information panes
        self.pane_registry
            .with_pane_mut(&PaneId::Diff, |p| p.set_visible(false));
        self.pane_registry
            .with_pane_mut(&PaneId::SideBySideDiff, |p| p.set_visible(false));
        self.pane_registry
            .with_pane_mut(&PaneId::Help, |p| p.set_visible(false));
        // Update the legacy field for consistency
        self.current_information_pane = InformationPane::Advice;
    }

    pub fn update_llm_advice(&mut self, advice: String) {
        self.llm_advice = advice.clone();
        self.pane_registry.with_pane_mut(&PaneId::Advice, |p| {
            let _ = p.handle_event(&crate::pane::AppEvent::DataUpdated((), advice));
        });
    }

    #[allow(dead_code)]
    pub fn get_llm_advice(&self) -> &str {
        &self.llm_advice
    }

    // Public getters for private fields needed by panes
    pub fn get_tree_nodes(&self) -> &Vec<(TreeNode, usize)> {
        &self.tree_nodes
    }

    pub fn get_current_tree_index(&self) -> usize {
        self.current_tree_index
    }

    pub fn get_files(&self) -> &Vec<FileDiff> {
        &self.files
    }

    pub fn get_file_change_timestamps(&self) -> &Vec<std::time::Instant> {
        &self.file_change_timestamps
    }

    pub fn get_monitor_command_configured(&self) -> bool {
        self.monitor_command_configured
    }

    pub fn get_monitor_has_run(&self) -> bool {
        self.monitor_has_run
    }

    pub fn get_monitor_elapsed_time(&self) -> Option<std::time::Duration> {
        self.monitor_elapsed_time
    }

    pub fn get_scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn is_file_recently_changed(&self, file_index: usize) -> bool {
        if let Some(timestamp) = self.file_change_timestamps.get(file_index) {
            timestamp.elapsed().as_secs() < 3
        } else {
            false
        }
    }

    pub fn get_last_active_pane(&self) -> ActivePane {
        self.last_active_pane
    }

    pub fn poll_llm_advice(&mut self) {
        self.pane_registry
            .with_pane_mut(&PaneId::Advice, |pane| {
                if let Some(advice_pane) = pane.as_advice_pane_mut() {
                    advice_pane.poll_llm_response();
                }
            });
    }

    pub fn is_advice_refresh_requested(&self) -> bool {
        if let Some(pane) = self.pane_registry.get_pane(&PaneId::Advice) {
            if let Some(advice_pane) = pane.as_advice_pane() {
                return advice_pane.refresh_requested;
            }
        }
        false
    }

    pub fn reset_advice_refresh_request(&mut self) {
        self.pane_registry
            .with_pane_mut(&PaneId::Advice, |pane| {
                if let Some(advice_pane) = pane.as_advice_pane_mut() {
                    advice_pane.refresh_requested = false;
                }
            });
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

    // Render status bar using new pane system
    app.pane_registry
        .render(f, app, chunks[0], PaneId::StatusBar, git_repo);

    // Handle the information pane (right side)
    let file_browser_visible = app.is_showing_changed_files_pane();
    let info_pane_visible = app.is_showing_diff_panel();

    match (file_browser_visible, info_pane_visible) {
        (true, true) => {
            // Both panes visible: split screen
            let bottom_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
                .split(chunks[1]);

            render_file_browser_pane(f, app, bottom_chunks[0], git_repo);

            let diff_height = bottom_chunks[1].height.saturating_sub(2) as usize;
            render_information_pane(f, app, bottom_chunks[1], diff_height, git_repo);
        }
        (true, false) => {
            // Only file browser visible
            render_file_browser_pane(f, app, chunks[1], git_repo);
        }
        (false, true) => {
            // Only information pane visible
            let diff_height = chunks[1].height.saturating_sub(2) as usize;
            render_information_pane(f, app, chunks[1], diff_height, git_repo);
        }
        (false, false) => {
            // Both hidden, render a blank block
            let block = Block::default()
                .title("Nothing to show")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.get_theme().border_color()));
            f.render_widget(block, chunks[1]);
        }
    }
}

fn render_file_browser_pane(f: &mut Frame, app: &App, area: Rect, git_repo: &GitRepo) {
    // If help is showing and diff panel is hidden, help takes over the full area
    if app.is_showing_help() && !app.is_showing_diff_panel() {
        app.pane_registry
            .render(f, app, area, PaneId::Help, git_repo);
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
                render_file_tree_content(f, app, tree_chunks[0], git_repo);

                // Render monitor pane in bottom half using new pane system
                app.pane_registry
                    .render(f, app, tree_chunks[1], PaneId::Monitor, git_repo);
            } else {
                // Monitor pane is hidden, file tree takes full area
                render_file_tree_content(f, app, area, git_repo);
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
            render_file_tree_content(f, app, tree_chunks[0], git_repo);

            // Render monitor pane in bottom half using new pane system
            app.pane_registry
                .render(f, app, tree_chunks[1], PaneId::Monitor, git_repo);
        }
    }
}

// Old InformationPaneRenderer trait replaced by new Pane trait system

fn render_information_pane(
    f: &mut Frame,
    app: &App,
    area: Rect,
    _max_lines: usize,
    git_repo: &GitRepo,
) {
    let help_visible = app
        .pane_registry
        .get_pane(&PaneId::Help)
        .is_some_and(|p| p.visible());
    let advice_visible = app
        .pane_registry
        .get_pane(&PaneId::Advice)
        .is_some_and(|p| p.visible());

    if help_visible {
        app.pane_registry
            .render(f, app, area, PaneId::Help, git_repo);
    } else if advice_visible {
        app.pane_registry
            .render(f, app, area, PaneId::Advice, git_repo);
    } else if app.side_by_side_diff {
        app.pane_registry
            .render(f, app, area, PaneId::SideBySideDiff, git_repo);
    } else {
        app.pane_registry
            .render(f, app, area, PaneId::Diff, git_repo);
    }
}

fn render_file_tree_content(f: &mut Frame, app: &App, area: Rect, _git_repo: &GitRepo) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;
    use std::env;

    fn create_test_app(show_diff_panel: bool, show_changed_files_pane: bool, theme: Theme) -> App {
        let mut llm_config = LlmConfig::default();
        if env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = LlmClient::new(llm_config).ok();
        App::new_with_config(show_diff_panel, show_changed_files_pane, theme, llm_client)
    }

    #[test]
    fn test_app_creation() {
        let app = create_test_app(true, true, Theme::Dark);
        assert_eq!(app.files.len(), 0);
        assert_eq!(app.current_file_index, 0);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.current_diff_height, 20);
        assert!(!app.is_showing_help());
        assert!(app.show_diff_panel);
        assert!(app.show_changed_files_pane);
        assert_eq!(app.current_file_browser_pane, FileBrowserPane::FileTree);
        assert_eq!(app.current_information_pane, InformationPane::Diff);
        assert!(!app.monitor_command_configured);
        assert!(app.monitor_elapsed_time.is_none());
        assert!(!app.monitor_has_run);
        assert_eq!(app.get_theme(), Theme::Dark);
        assert_eq!(app.last_active_pane, ActivePane::default());
    }

    #[test]
    fn test_app_creation_no_diff() {
        let app = create_test_app(false, true, Theme::Dark);
        assert!(!app.show_diff_panel);
    }

    #[test]
    fn test_scroll_up() {
        let mut app = create_test_app(true, true, Theme::Dark);
        app.scroll_offset = 5;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 4);
    }

    #[test]
    fn test_scroll_up_at_zero() {
        let mut app = create_test_app(true, true, Theme::Dark);
        app.scroll_offset = 0;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_page_up() {
        let mut app = create_test_app(true, true, Theme::Dark);
        app.scroll_offset = 25;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 15);
    }

    #[test]
    fn test_page_up_underflow() {
        let mut app = create_test_app(true, true, Theme::Dark);
        app.scroll_offset = 5;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_to_top() {
        let mut app = create_test_app(true, true, Theme::Dark);
        app.scroll_offset = 100;
        app.scroll_to_top();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_toggle_help() {
        let mut app = create_test_app(true, true, Theme::Dark);
        assert!(!app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Diff);

        app.toggle_help();
        assert!(app.is_showing_help());
        // Check that help pane is visible through the pane registry
        assert!(&app.pane_registry.get_pane(&PaneId::Help).unwrap().visible());

        app.toggle_help();
        assert!(!app.is_showing_help());
        // Check that help pane is hidden through the pane registry
        assert!(!&app.pane_registry.get_pane(&PaneId::Help).unwrap().visible());
    }

    #[test]
    fn test_toggle_diff_panel() {
        let mut app = create_test_app(true, true, Theme::Dark);
        assert!(app.show_diff_panel);
        app.toggle_diff_panel();
        assert!(!app.show_diff_panel);
        app.toggle_diff_panel();
        assert!(app.show_diff_panel);
    }

    #[test]
    fn test_toggle_changed_files_pane() {
        let mut app = create_test_app(true, true, Theme::Dark);
        assert!(app.is_showing_changed_files_pane());
        app.toggle_changed_files_pane();
        assert!(!app.is_showing_changed_files_pane());
        app.toggle_changed_files_pane();
        assert!(app.is_showing_changed_files_pane());
    }

    #[test]
    fn test_monitor_output_update() {
        let mut app = create_test_app(true, true, Theme::Dark);
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
        let mut app = create_test_app(true, true, Theme::Dark);

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
        let mut app = create_test_app(true, true, Theme::Dark);

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
        let mut app = create_test_app(true, true, Theme::Dark);

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
        let mut app = create_test_app(true, true, Theme::Dark);

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
        let app = create_test_app(true, true, Theme::Dark);

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
        let mut app = create_test_app(true, true, Theme::Dark);

        // Initially in single-pane diff mode
        assert_eq!(app.current_information_pane, InformationPane::Diff);
        assert!(!app.is_side_by_side_diff());

        // Switch to side-by-side mode
        app.set_side_by_side_diff();
        // Check through pane registry
        assert!(
            &app.pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );
        assert!(!&app.pane_registry.get_pane(&PaneId::Diff).unwrap().visible());
        assert!(app.is_side_by_side_diff());

        // Switch back to single-pane mode
        app.set_single_pane_diff();
        // Check through pane registry
        assert!(
            !&app
                .pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );
        assert!(&app.pane_registry.get_pane(&PaneId::Diff).unwrap().visible());
        assert!(!app.is_side_by_side_diff());
    }

    #[test]
    fn test_help_preserves_diff_mode() {
        let mut app = create_test_app(true, true, Theme::Dark);

        // Set to side-by-side mode
        app.set_side_by_side_diff();
        assert!(
            &app.pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );

        // Show help
        app.toggle_help();
        assert!(&app.pane_registry.get_pane(&PaneId::Help).unwrap().visible());
        assert!(
            !&app
                .pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );

        // Hide help - should return to side-by-side mode
        app.toggle_help();
        assert!(!&app.pane_registry.get_pane(&PaneId::Help).unwrap().visible());
        assert!(
            &app.pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );
    }

    #[test]
    fn test_help_movement_when_diff_panel_hidden() {
        let mut app = create_test_app(true, true, Theme::Dark);

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

    // Test that help works when both file tree and diff panes are visible
    #[test]
    fn test_help_with_both_panes_visible() {
        let mut app = create_test_app(true, true, Theme::Dark);

        // Initially both file tree and diff panels should be showing
        assert!(app.is_showing_diff_panel());
        assert!(!app.is_showing_help());

        // Show help while both panes are visible
        app.toggle_help();
        assert!(app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Help);

        // Help should be visible via the pane registry
        assert!(app.pane_registry.get_pane(&PaneId::Help).unwrap().visible());

        // Diff panes should be hidden while help is showing
        assert!(!app.pane_registry.get_pane(&PaneId::Diff).unwrap().visible());
        assert!(
            !app.pane_registry
                .get_pane(&PaneId::SideBySideDiff)
                .unwrap()
                .visible()
        );

        // Hide help
        app.toggle_help();
        assert!(!app.is_showing_help());
        assert_eq!(app.current_information_pane, InformationPane::Diff);

        // Diff pane should be visible again
        assert!(app.pane_registry.get_pane(&PaneId::Diff).unwrap().visible());
    }

    #[test]
    fn test_theme_toggle() {
        let mut app = create_test_app(true, true, Theme::Dark);

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
        assert_eq!(light_theme.added_color(), Color::Green);
        assert_eq!(light_theme.removed_color(), Color::LightRed);
    }
}
