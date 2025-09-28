use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::App;

#[derive(Debug, Clone, PartialEq)]
pub enum CommitPickerLoadingState {
    NotLoaded,
    Loading,
    Loaded,
    Error,
}

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

impl Default for CommitPickerPane {
    fn default() -> Self {
        Self::new()
    }
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::AppEvent;
    use crossterm::event::{KeyEvent, KeyModifiers};

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
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
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
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
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
}
