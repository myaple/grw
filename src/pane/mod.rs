use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};

use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::shared_state::LlmSharedState;
use crate::ui::{App, Theme};

// Module declarations
mod advice_panel;
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
pub use advice_panel::*;
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
    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePanel> {
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
    Advice,
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
    pub fn new(theme: Theme, llm_client: LlmClient, llm_shared_state: Arc<LlmSharedState>) -> Self {
        let mut registry = Self {
            panes: HashMap::new(),
            theme,
        };

        registry.register_default_panes(llm_client, llm_shared_state);
        registry
    }

    fn register_default_panes(
        &mut self,
        llm_client: LlmClient,
        llm_shared_state: Arc<LlmSharedState>,
    ) {
        self.register_pane(PaneId::FileTree, Box::new(FileTreePane::new()));
        self.register_pane(PaneId::Monitor, Box::new(MonitorPane::new()));
        self.register_pane(PaneId::Diff, Box::new(DiffPane::new()));
        self.register_pane(PaneId::SideBySideDiff, Box::new(SideBySideDiffPane::new()));
        self.register_pane(PaneId::Help, Box::new(HelpPane::new()));
        self.register_pane(PaneId::StatusBar, Box::new(StatusBarPane::new()));
        self.register_pane(PaneId::CommitPicker, Box::new(CommitPickerPane::new()));
        let mut commit_summary_pane =
            CommitSummaryPane::new_with_llm_client(Some(llm_client.clone()));
        commit_summary_pane.set_shared_state(llm_shared_state.clone());
        self.register_pane(PaneId::CommitSummary, Box::new(commit_summary_pane));

        // Create advice panel with configuration, LLM client, and shared state
        let _advice_config = crate::config::AdviceConfig::default();
        let mut advice_panel = AdvicePanel::new().expect("Failed to create AdvicePanel");
        advice_panel.set_shared_state(llm_shared_state.clone());

        // Extract max_tokens from LlmClient config and set it
        let max_tokens = llm_client.get_max_tokens();
        advice_panel.set_max_tokens(max_tokens);

        advice_panel.set_llm_client(std::sync::Arc::new(tokio::sync::Mutex::new(
            llm_client.clone(),
        )));
        self.register_pane(PaneId::Advice, Box::new(advice_panel));
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
    use crate::config::LlmConfig;
    use std::env;
    use std::sync::Arc;

    fn create_test_pane_registry() -> PaneRegistry {
        let mut llm_config = LlmConfig::default();
        if env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = crate::llm::LlmClient::new(llm_config).unwrap();
        let llm_shared_state = Arc::new(crate::shared_state::LlmSharedState::new());
        PaneRegistry::new(crate::ui::Theme::Dark, llm_client, llm_shared_state)
    }

    #[test]
    fn test_pane_registry_creation() {
        let registry = create_test_pane_registry();
        assert_eq!(registry.panes.len(), 9); // Default panes + commit picker + commit summary + advice pane
        assert!(registry.get_pane(&PaneId::FileTree).is_some());
        assert!(registry.get_pane(&PaneId::Monitor).is_some());
        assert!(registry.get_pane(&PaneId::Diff).is_some());
        assert!(registry.get_pane(&PaneId::CommitPicker).is_some());
        assert!(registry.get_pane(&PaneId::CommitSummary).is_some());
        assert!(registry.get_pane(&PaneId::Advice).is_some());
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

    #[test]
    fn test_panel_opening_integration_contract() {
        // Test integration contract for panel opening flow
        use crate::ui::App;
        use std::sync::Arc;

        // Create app using the same pattern as existing tests
        let mut llm_config = crate::config::LlmConfig::default();
        if std::env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = crate::llm::LlmClient::new(llm_config).ok();
        let llm_state = Arc::new(crate::shared_state::LlmSharedState::new());
        let mut app =
            App::new_with_config(true, true, crate::ui::Theme::Dark, llm_client, llm_state);

        // Test that toggle_pane_visibility works for advice panel
        // This is the core integration contract - Ctrl+L should work
        let result1 = app.toggle_pane_visibility(&PaneId::Advice);
        assert!(
            result1.is_ok(),
            "Should be able to toggle advice panel visibility"
        );

        // Test toggling back
        let result2 = app.toggle_pane_visibility(&PaneId::Advice);
        assert!(
            result2.is_ok(),
            "Should be able to toggle advice panel back to hidden"
        );

        // Test error handling for invalid pane ID
        let invalid_result = app.toggle_pane_visibility(&PaneId::FileTree); // FileTree is not togglable
        // This should either succeed or give a meaningful error
        assert!(
            invalid_result.is_ok(),
            "Should handle invalid pane gracefully"
        );
    }

    #[test]
    fn test_panel_opening_keyboard_integration_contract() {
        // Test contract for keyboard integration - this represents what happens in main.rs
        // We can't directly test key events here, but we can test the method that gets called
        use crate::ui::App;
        use std::sync::Arc;

        // Create app with same pattern as tests
        let mut llm_config = crate::config::LlmConfig::default();
        if std::env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = crate::llm::LlmClient::new(llm_config).ok();
        let llm_state = Arc::new(crate::shared_state::LlmSharedState::new());
        let mut app =
            App::new_with_config(true, true, crate::ui::Theme::Dark, llm_client, llm_state);

        // Test multiple toggles to simulate repeated Ctrl+L presses
        for i in 0..5 {
            let result = app.toggle_pane_visibility(&PaneId::Advice);
            assert!(result.is_ok(), "Ctrl+L toggle {} should succeed", i + 1);
        }
    }
}
