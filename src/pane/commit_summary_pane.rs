use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};
use std::sync::Arc;

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::shared_state::LlmSharedState;
use crate::ui::App;

#[derive(Debug, Clone, PartialEq)]
pub enum CommitSummaryLoadingState {
    NoCommit,
    Loaded,
}

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

impl Default for CommitSummaryPane {
    fn default() -> Self {
        Self::new()
    }
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

    pub fn clear_error(&mut self) {
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.clear_error("commit_summary");
        }
    }

    fn request_llm_summary(&mut self) {
        if let Some(_commit) = &self.current_commit {
            self.loading_state = CommitSummaryLoadingState::Loaded;
            self.llm_summary =
                Some("LLM summary generation not yet implemented with shared state".to_string());
        }
    }

    /// Set a cached summary directly without generating a new one
    pub fn set_cached_summary(&mut self, commit_sha: &str, summary: String) {
        if let Some(current_commit) = &self.current_commit
            && current_commit.sha == commit_sha
        {
            self.llm_summary = Some(summary);
            self.clear_error();
            self.is_loading_summary = false;
            self.pending_summary_sha = None;
            self.loading_state = CommitSummaryLoadingState::Loaded;
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
        let theme = app.get_theme();

        // Handle different loading states
        if self.loading_state == CommitSummaryLoadingState::NoCommit {
            let paragraph = Paragraph::new("No commit selected").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
            return Ok(());
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

    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        Some(self)
    }
}
