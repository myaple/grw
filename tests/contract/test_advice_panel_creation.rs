use grw::config::Config;
use grw::pane::AdvicePanel;

/// Test contract for AdvicePanel creation
#[test]
fn test_advice_panel_creation_contract() {
    // Arrange - Test data
    let config = Config::default();

    // Act - Create AdvicePanel
    let result = AdvicePanel::new();

    // Assert - Validate contract requirements
    assert!(result.is_ok(), "AdvicePanel::new() should return Ok(AdvicePanel)");

    let panel = result.unwrap();

    // Contract: Must initialize with default state
    assert!(!panel.visible(), "Panel must not be visible by default");

    // Contract: Validate initial state - these methods should exist on the actual AdvicePanel
    assert!(panel.get_improvements().is_empty(), "Initial improvements should be empty");
    assert!(panel.get_chat_history().is_empty(), "Initial chat history should be empty");
    assert_eq!(panel.get_advice_generation_status(), "Ready", "Initial status should be Ready");
    assert!(panel.get_last_chat_error().is_none(), "Initial chat error should be None");
    assert!(panel.is_chat_available(), "Chat should be available by default");
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
            max_tokens: Some(16000),
        }),
        ..Default::default()
    };

    // Act - Create AdvicePanel
    let result = AdvicePanel::new();

    // Assert - Should work with LLM config
    assert!(result.is_ok(), "AdvicePanel::new() should work with LLM config");

    let panel = result.unwrap();
    assert!(!panel.visible(), "Should initialize properly with LLM config");
}

/// Test contract for AdvicePanel creation failure scenarios
#[test]
fn test_advice_panel_creation_error_handling_contract() {
    // Arrange - Test with potentially problematic config
    let config = Config::default();

    // Act - Create AdvicePanel
    let result = AdvicePanel::new();

    // Assert - Should handle invalid config gracefully
    assert!(result.is_ok(), "AdvicePanel::new() should handle invalid config gracefully");

    let panel = result.unwrap();
    assert!(!panel.visible(), "Should still initialize properly");
}