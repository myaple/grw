use crossterm::event::KeyCode;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::App;

pub struct FileTreePane {
    visible: bool,
    scroll_offset: usize,
}

impl Default for FileTreePane {
    fn default() -> Self {
        Self::new()
    }
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
