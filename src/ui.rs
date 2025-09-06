use crate::git::{FileDiff, GitRepo, TreeNode};
use ratatui::{
    Frame,
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
};

#[derive(Debug)]
pub struct App {
    files: Vec<FileDiff>,
    current_file_index: usize,
    scroll_offset: usize,
    tree_nodes: Vec<(TreeNode, usize)>,
    current_tree_index: usize,
    file_indices_in_tree: Vec<usize>,
    pub last_g_press: Option<std::time::Instant>,
    show_help: bool,
    pub current_diff_height: usize,
}

impl App {
    pub fn new() -> Self {
        Self {
            files: Vec::new(),
            current_file_index: 0,
            scroll_offset: 0,
            tree_nodes: Vec::new(),
            current_tree_index: 0,
            file_indices_in_tree: Vec::new(),
            last_g_press: None,
            show_help: false,
            current_diff_height: 20,
        }
    }

    pub fn update_files(&mut self, files: Vec<FileDiff>) {
        self.files = files;
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
        self.show_help = !self.show_help;
    }

    pub fn is_showing_help(&self) -> bool {
        self.show_help
    }

    pub fn next_file(&mut self) {
        if !self.files.is_empty() {
            // Find the next file in the tree that has a valid file index
            let start_tree_index = self.current_tree_index;
            let mut next_tree_index = (self.current_tree_index + 1) % self.tree_nodes.len();

            // Look for the next tree node that represents a file
            while next_tree_index != start_tree_index {
                if let Some(&file_idx) = self.file_indices_in_tree.get(next_tree_index) {
                    if file_idx != usize::MAX {
                        self.current_file_index = file_idx;
                        self.current_tree_index = next_tree_index;
                        self.scroll_offset = 0;
                        return;
                    }
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
                if let Some(&file_idx) = self.file_indices_in_tree.get(prev_tree_index) {
                    if file_idx != usize::MAX {
                        self.current_file_index = file_idx;
                        self.current_tree_index = prev_tree_index;
                        self.scroll_offset = 0;
                        return;
                    }
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

    pub fn get_file_count(&self) -> usize {
        self.files.len()
    }
}

pub fn render<B: Backend>(f: &mut Frame, app: &App, git_repo: &GitRepo) {
    let size = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)])
        .split(size);

    let bottom_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Calculate available height for diff content
    let diff_height = bottom_chunks[1].height.saturating_sub(2) as usize;

    render_status_bar::<B>(f, git_repo, chunks[0]);
    render_file_tree::<B>(f, app, bottom_chunks[0]);
    render_diff_view::<B>(f, app, bottom_chunks[1], diff_height);
}

fn render_file_tree<B: Backend>(f: &mut Frame, app: &App, area: Rect) {
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

                let status_char = if let Some(ref diff) = node.file_diff {
                    if diff.status.is_wt_new() {
                        "📄 "
                    } else if diff.status.is_wt_modified() {
                        "📝 "
                    } else if diff.status.is_wt_deleted() {
                        "🗑️  "
                    } else {
                        "❓ "
                    }
                } else {
                    "❓ "
                };

                spans.push(Span::raw(format!("{}{}", indent, status_char)));
                spans.push(Span::raw(node.name.clone()));

                if let Some(ref diff) = node.file_diff {
                    if diff.additions > 0 {
                        spans.push(Span::styled(
                            format!(" (+{})", diff.additions),
                            Style::default()
                                .fg(Color::Green)
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    if diff.deletions > 0 {
                        spans.push(Span::styled(
                            format!(" (-{})", diff.deletions),
                            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                        ));
                    }
                }

                spans
            };

            let line = if index == app.current_tree_index {
                Line::from(name_spans).style(
                    Style::default()
                        .fg(Color::Yellow)
                        .bg(Color::DarkGray)
                        .add_modifier(Modifier::BOLD),
                )
            } else if let Some(ref _diff) = node.file_diff {
                Line::from(name_spans).style(Style::default().fg(Color::White))
            } else {
                Line::from(name_spans).style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )
            };

            ListItem::new(line)
        })
        .collect();

    let file_list = List::new(tree_items)
        .block(
            Block::default()
                .title("Changed Files")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Gray)),
        )
        .highlight_style(
            Style::default()
                .fg(Color::Yellow)
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_widget(file_list, area);
}

fn render_status_bar<B: Backend>(f: &mut Frame, git_repo: &GitRepo, area: Rect) {
    let repo_name = git_repo.get_repo_name();
    let branch = git_repo.get_current_branch();
    let (commit_sha, commit_summary) = git_repo.get_last_commit_info();
    let (total_files, total_additions, total_deletions) = git_repo.get_total_stats();

    // Calculate available space for commit message
    let left_part = format!("📁 {} | 🌿 {} | 🎯 {} | ", repo_name, branch, commit_sha);
    let right_part = format!(
        " | 📊 {} files (+{}/-{})",
        total_files, total_additions, total_deletions
    );

    let available_space = area.width as usize;
    let left_len = left_part.len();
    let right_len = right_part.len();

    let commit_message_space = available_space.saturating_sub(left_len + right_len);
    let truncated_summary = if commit_summary.len() > commit_message_space {
        format!(
            "{}...",
            &commit_summary[..commit_message_space.saturating_sub(3)]
        )
    } else {
        commit_summary
    };

    let status_text = format!("{}{}{}", left_part, truncated_summary, right_part);

    let paragraph = Paragraph::new(status_text)
        .style(Style::default().add_modifier(Modifier::REVERSED))
        .block(Block::default().borders(Borders::NONE))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

fn render_diff_view<B: Backend>(f: &mut Frame, app: &App, area: Rect, max_lines: usize) {
    if app.is_showing_help() {
        render_help_view::<B>(f, area);
    } else if let Some(file) = app.get_current_file() {
        let file_path = file.path.to_string_lossy();
        let title = format!("Diff: {}", file_path);

        let mut lines = Vec::new();

        for (i, line) in file.line_strings.iter().enumerate() {
            if i < app.scroll_offset {
                continue;
            }

            if lines.len() >= max_lines {
                break;
            }

            let (style, line_text) = if line.starts_with('+') {
                (Style::default().fg(Color::Green), line)
            } else if line.starts_with('-') {
                (Style::default().fg(Color::Red), line)
            } else if line.starts_with(' ') {
                (Style::default().fg(Color::Gray), line)
            } else {
                (Style::default().fg(Color::White), line)
            };

            let span = Span::styled(line_text.clone(), style);
            lines.push(Line::from(span));
        }

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text)
            .block(Block::default().title(title).borders(Borders::ALL))
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
    } else {
        let paragraph = Paragraph::new("No changes detected")
            .block(Block::default().title("Diff").borders(Borders::ALL));
        f.render_widget(paragraph, area);
    }
}

fn render_help_view<B: Backend>(f: &mut Frame, area: Rect) {
    let help_text = vec![
        Line::from(Span::styled(
            "Git Repository Watcher - Help",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Navigation:",
            Style::default()
                .fg(Color::Cyan)
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
                .fg(Color::Cyan)
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
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  ?             - Show/hide this help page"),
        Line::from("  Esc           - Exit help page"),
        Line::from("  q / Ctrl+C    - Quit application"),
        Line::from(""),
        Line::from("Press ? or Esc to return to diff view"),
    ];

    let text = Text::from(help_text);
    let paragraph = Paragraph::new(text)
        .block(Block::default().title("Help").borders(Borders::ALL))
        .wrap(Wrap { trim: false });

    f.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = App::new();
        assert_eq!(app.files.len(), 0);
        assert_eq!(app.current_file_index, 0);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.current_diff_height, 20);
        assert!(!app.show_help);
    }

    #[test]
    fn test_scroll_up() {
        let mut app = App::new();
        app.scroll_offset = 5;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 4);
    }

    #[test]
    fn test_scroll_up_at_zero() {
        let mut app = App::new();
        app.scroll_offset = 0;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_page_up() {
        let mut app = App::new();
        app.scroll_offset = 25;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 15);
    }

    #[test]
    fn test_page_up_underflow() {
        let mut app = App::new();
        app.scroll_offset = 5;
        app.page_up(10);
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_scroll_to_top() {
        let mut app = App::new();
        app.scroll_offset = 100;
        app.scroll_to_top();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_toggle_help() {
        let mut app = App::new();
        assert!(!app.show_help);
        app.toggle_help();
        assert!(app.show_help);
        app.toggle_help();
        assert!(!app.show_help);
    }
}
