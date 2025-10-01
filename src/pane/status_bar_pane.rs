use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::ui::App;

pub struct StatusBarPane {
    visible: bool,
}

impl Default for StatusBarPane {
    fn default() -> Self {
        Self::new()
    }
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
                "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {} > {} | 📊 {} files (+{}/-{})",
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
