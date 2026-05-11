use crossterm::event::{KeyCode, KeyModifiers};
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
pub enum CommitSummaryLoadingState {
    NoCommit,
    Loaded,
}

pub struct CommitSummaryPane {
    visible: bool,
    current_commit: Option<crate::git::CommitInfo>,
    scroll_offset: usize,
    loading_state: CommitSummaryLoadingState,
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
            self.scroll_offset = 0;

            // Update loading state based on new commit
            if self.current_commit.is_some() {
                self.loading_state = CommitSummaryLoadingState::Loaded;
            } else {
                self.loading_state = CommitSummaryLoadingState::NoCommit;
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

            // Render file changes section (full area since no LLM summary section)
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

                    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
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

            f.render_widget(file_list, area);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::AppEvent;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn test_commit_summary_pane_creation() {
        let pane = CommitSummaryPane::new();
        assert!(!pane.visible());
        assert!(pane.current_commit.is_none());
        assert_eq!(pane.scroll_offset, 0);
    }

    #[test]
    fn test_commit_summary_pane_update_commit() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
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
    }

    #[test]
    fn test_commit_summary_pane_scrolling() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
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
}