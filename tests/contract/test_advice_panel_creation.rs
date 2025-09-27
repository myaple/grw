#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{AdvicePanel, Pane, PaneId};
    use crate::config::{AdviceConfig, Config};

    #[test]
    fn test_advice_panel_creation() {
        // Test that AdvicePanel can be created with default configuration
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config);

        assert!(panel.is_ok(), "Failed to create AdvicePanel");

        let panel = panel.unwrap();
        assert_eq!(panel.id(), PaneId::Advice);
        assert!(panel.is_visible());
    }

    #[test]
    fn test_advice_panel_with_custom_config() {
        // Test AdvicePanel creation with custom configuration
        let mut config = Config::default();
        config.advice = Some(AdviceConfig {
            enabled: Some(true),
            advice_model: Some("gpt-4o".to_string()),
            max_improvements: Some(5),
            chat_history_limit: Some(20),
            timeout_seconds: Some(60),
            context_lines: Some(100),
        });

        let advice_config = config.advice.as_ref().unwrap().clone();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config);

        assert!(panel.is_ok(), "Failed to create AdvicePanel with custom config");

        let panel = panel.unwrap();
        assert_eq!(panel.id(), PaneId::Advice);
    }

    #[test]
    fn test_advice_panel_disabled_config() {
        // Test AdvicePanel behavior when disabled in config
        let mut config = Config::default();
        config.advice = Some(AdviceConfig {
            enabled: Some(false),
            ..Default::default()
        });

        let advice_config = config.advice.as_ref().unwrap().clone();

        // This should fail until AdvicePanel is implemented
        let result = AdvicePanel::new(config, advice_config);

        // Panel should still be creatable but might have limited functionality
        assert!(result.is_ok(), "Should be able to create AdvicePanel even when disabled");
    }

    #[test]
    fn test_advice_panel_required_methods() {
        // Test that AdvicePanel implements required Pane trait methods
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test required trait methods exist and work
        assert_eq!(panel.id(), PaneId::Advice);
        assert!(panel.is_visible());

        // Test render method exists (signature check)
        let mut frame = ratatui::Frame::default();

        // This should fail until render is implemented
        let render_result = std::panic::catch_unwind(|| {
            panel.render(&mut frame, ratatui::layout::Rect::default());
        });

        // For now, expect it to panic or fail since not implemented
        assert!(render_result.is_err(), "Render method should not be implemented yet");
    }

    #[test]
    fn test_advice_panel_initial_state() {
        // Test AdvicePanel initial state after creation
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Panel should start in viewing mode with no chat history
        // This will fail until the state management is implemented
        let mode = panel.get_mode();
        assert_eq!(mode, crate::pane::AdviceMode::Viewing);

        let chat_history = panel.get_chat_history();
        assert!(chat_history.is_empty(), "Initial chat history should be empty");

        let improvements = panel.get_improvements();
        assert!(improvements.is_empty(), "Initial improvements should be empty");
    }
}