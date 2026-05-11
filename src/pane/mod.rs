use std::collections::HashMap;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::git::GitRepo;
use crate::ui::{App, Theme};

// Module declarations
mod commit_picker_pane;
mod commit_summary_pane;
mod diff_pane;
mod file_tree_pane;
mod help_pane;
mod keys;
mod monitor_pane;
mod side_by_side_diff_pane;
mod status_bar_pane;

// Re-exports to maintain public API
pub use commit_picker_pane::*;
pub use commit_summary_pane::*;
pub use diff_pane::*;
pub use file_tree_pane::*;
pub use help_pane::*;
pub use keys::*;
pub use monitor_pane::*;
pub use side_by_side_diff_pane::*;
pub use status_bar_pane::*;

// Core trait that all panes implement
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
    fn as_commit_picker_pane(&self) -> Option<&CommitPickerPane> {
        None
    }
    fn as_commit_picker_pane_mut(&mut self) -> Option<&mut CommitPickerPane> {
        None
    }
    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        None
    }
}

// Shared enums and types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    FileTree,
    Monitor,
    Diff,
    SideBySideDiff,
    Help,
    StatusBar,
    CommitPicker,
    CommitSummary,
}

#[derive(Debug, Clone)]
pub enum AppEvent {
    Key(KeyEvent),
    DataUpdated((), String),
    ThemeChanged(()),
}

// PaneRegistry - Central registry for managing panes
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
        self.register_pane(PaneId::CommitPicker, Box::new(CommitPickerPane::new()));
        let commit_summary_pane = CommitSummaryPane::new();
        self.register_pane(PaneId::CommitSummary, Box::new(commit_summary_pane));
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

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_pane_registry() -> PaneRegistry {
        PaneRegistry::new(crate::ui::Theme::Dark)
    }

    #[test]
    fn test_pane_registry_creation() {
        let registry = create_test_pane_registry();
        assert_eq!(registry.panes.len(), 8); // Default panes
        assert!(registry.get_pane(&PaneId::FileTree).is_some());
        assert!(registry.get_pane(&PaneId::Monitor).is_some());
        assert!(registry.get_pane(&PaneId::Diff).is_some());
        assert!(registry.get_pane(&PaneId::CommitPicker).is_some());
        assert!(registry.get_pane(&PaneId::CommitSummary).is_some());
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
}