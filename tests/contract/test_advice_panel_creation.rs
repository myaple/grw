use grw::config::{AdviceConfig, Config};
use grw::pane::{AdvicePanel, AdviceMode};

/// Test contract for AdvicePanel creation
#[test]
fn test_advice_panel_creation_contract() {
    // Arrange - Test data
    let config = Config::default();
    let advice_config = AdviceConfig::default();

    // Act - Create AdvicePanel
    let result = AdvicePanel::new(config, advice_config);

    // Assert - Validate contract requirements
    assert!(result.is_ok(), "AdvicePanel::new() should return Ok(AdvicePanel)");

    let panel = result.unwrap();

    // Contract: Must initialize with default state
    assert!(!panel.visible(), "Panel must not be visible by default");
    assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Must start in Viewing mode");

    // Contract: Validate initial state
    assert!(panel.get_improvements().is_empty(), "Initial improvements should be empty");
    assert!(panel.get_chat_history().is_empty(), "Initial chat history should be empty");
    assert_eq!(panel.get_advice_generation_status(), "Ready", "Initial status should be Ready");
    assert!(panel.get_last_chat_error().is_none(), "Initial chat error should be None");
    assert!(panel.is_chat_available(), "Chat should be available by default");
}

/// Test contract for AdvicePanel creation with custom configuration
#[test]
fn test_advice_panel_creation_with_custom_config_contract() {
    // Arrange - Custom configuration
    let config = Config::default();
    let advice_config = AdviceConfig {
        enabled: Some(true),
        advice_model: Some("gpt-4o".to_string()),
        max_improvements: Some(5),
        chat_history_limit: Some(20),
        timeout_seconds: Some(60),
        context_lines: Some(100),
    };

    // Act - Create AdvicePanel with custom config
    let result = AdvicePanel::new(config, advice_config);

    // Assert - Validate contract requirements
    assert!(result.is_ok(), "AdvicePanel::new() should work with custom config");

    let panel = result.unwrap();

    // Contract: Should still initialize with default state regardless of config
    assert!(!panel.visible(), "Panel must not be visible by default even with custom config");
    assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Must start in Viewing mode");
}

/// Test contract for AdvicePanel creation failure scenarios
#[test]
fn test_advice_panel_creation_error_handling_contract() {
    // Arrange - Test with potentially problematic config
    let config = Config::default();

    // Test with invalid advice config (empty model)
    let advice_config = AdviceConfig {
        enabled: Some(true),
        advice_model: Some("".to_string()), // Empty model should be handled gracefully
        max_improvements: Some(0), // Invalid max_improvements
        chat_history_limit: Some(0), // Invalid chat_history_limit
        timeout_seconds: Some(0), // Invalid timeout
        context_lines: Some(0), // Invalid context_lines
    };

    // Act - Create AdvicePanel
    let result = AdvicePanel::new(config, advice_config);

    // Assert - Should handle invalid config gracefully
    assert!(result.is_ok(), "AdvicePanel::new() should handle invalid config gracefully");

    let panel = result.unwrap();
    assert!(!panel.visible(), "Should still initialize properly");
}

/// Test contract for AdvicePanel creation with LLM configuration
#[test]
fn test_advice_panel_creation_with_llm_config_contract() {
    // Arrange - Config with LLM settings
    let config = Config {
        llm: Some(grw::config::LlmConfig {
            provider: Some(grw::config::LlmProvider::OpenAI),
            model: Some("gpt-4o-mini".to_string()),
            advice_model: Some("gpt-4o".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: Some("https://api.openai.com/v1".to_string()),
            summary_model: None,
        }),
        ..Default::default()
    };

    let advice_config = AdviceConfig::default();

    // Act - Create AdvicePanel
    let result = AdvicePanel::new(config, advice_config);

    // Assert - Should work with LLM config
    assert!(result.is_ok(), "AdvicePanel::new() should work with LLM config");

    let panel = result.unwrap();
    assert!(!panel.visible(), "Should initialize properly with LLM config");
}

/// Test contract for AdvicePanel creation with disabled advice
#[test]
fn test_advice_panel_creation_with_disabled_advice_contract() {
    // Arrange - Disabled advice config
    let config = Config::default();
    let advice_config = AdviceConfig {
        enabled: Some(false),
        ..Default::default()
    };

    // Act - Create AdvicePanel
    let result = AdvicePanel::new(config, advice_config);

    // Assert - Should still create panel even when disabled
    assert!(result.is_ok(), "AdvicePanel::new() should work even when advice is disabled");

    let panel = result.unwrap();
    assert!(!panel.visible(), "Should initialize properly when disabled");
    assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should start in Viewing mode when disabled");
}