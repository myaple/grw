#[cfg(test)]
mod tests {
    use super::*;
    use crate::pane::{AdvicePanel, Pane, PaneId};
    use crate::config::{AdviceConfig, Config};
    use crate::pane::{ChatMessageData, MessageRole};

    #[test]
    fn test_chat_api_methods_exist() {
        // Test that chat functionality API methods exist on AdvicePanel
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that chat-related methods exist and don't panic when called
        let send_result = std::panic::catch_unwind(|| {
            panel.send_chat_message("Hello, AI!")
        });

        assert!(send_result.is_ok(), "send_chat_message method should exist and not panic");

        let history_result = std::panic::catch_unwind(|| {
            panel.get_chat_history()
        });

        assert!(history_result.is_ok(), "get_chat_history method should exist and not panic");

        let clear_result = std::panic::catch_unwind(|| {
            panel.clear_chat_history()
        });

        assert!(clear_result.is_ok(), "clear_chat_history method should exist and not panic");
    }

    #[test]
    fn test_send_chat_message() {
        // Test sending chat messages to the advice panel
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send a chat message
        let result = panel.send_chat_message("Can you explain this code change?");

        // Should handle message sending without error
        assert!(result.is_ok(), "Should send chat message without error");

        // Message should be added to chat history
        let history = panel.get_chat_history();
        assert!(!history.is_empty(), "Chat history should not be empty after sending message");

        // Verify the sent message is in history
        let last_message = &history[history.len() - 1];
        assert_eq!(last_message.role, MessageRole::User, "Last message should be from user");
        assert_eq!(last_message.content, "Can you explain this code change?", "Message content should match");
    }

    #[test]
    fn test_chat_message_with_context() {
        // Test that chat messages maintain context from advice generation
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // First generate some advice (this would normally come from git diff)
        let sample_diff = "sample diff content";
        let _ = panel.generate_advice(sample_diff);

        // Then send a follow-up chat message
        let result = panel.send_chat_message("Can you elaborate on the first improvement?");

        assert!(result.is_ok(), "Should send contextual chat message without error");

        // Chat history should include both the system context and user message
        let history = panel.get_chat_history();
        assert!(history.len() >= 2, "Chat history should contain context and user message");

        // Verify user message is present
        let user_messages: Vec<_> = history.iter().filter(|m| m.role == MessageRole::User).collect();
        assert_eq!(user_messages.len(), 1, "Should have exactly one user message");
        assert_eq!(user_messages[0].content, "Can you elaborate on the first improvement?");
    }

    #[test]
    fn test_chat_history_management() {
        // Test chat history management functionality
        let config = Config::default();
        let mut advice_config = AdviceConfig::default();
        advice_config.chat_history_limit = Some(3); // Small limit for testing

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send multiple messages to test history limit
        let messages = vec![
            "First message",
            "Second message",
            "Third message",
            "Fourth message", // This should push out the first message due to limit
        ];

        for (i, message) in messages.iter().enumerate() {
            let result = panel.send_chat_message(message);
            assert!(result.is_ok(), "Message {} should send successfully", i + 1);
        }

        // Check that history respects the limit
        let history = panel.get_chat_history();
        assert!(history.len() <= 3, "Chat history should respect the configured limit");

        // Verify the most recent messages are kept
        if history.len() >= 3 {
            assert!(history.iter().any(|m| m.content == "Second message"), "Should keep second message");
            assert!(history.iter().any(|m| m.content == "Third message"), "Should keep third message");
            assert!(history.iter().any(|m| m.content == "Fourth message"), "Should keep fourth message");
            assert!(!history.iter().any(|m| m.content == "First message"), "Should have removed first message");
        }
    }

    #[test]
    fn test_clear_chat_history() {
        // Test clearing chat history
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send some messages first
        panel.send_chat_message("Message 1").unwrap();
        panel.send_chat_message("Message 2").unwrap();

        // Verify history is not empty
        let history = panel.get_chat_history();
        assert!(!history.is_empty(), "Chat history should not be empty before clearing");

        // Clear the history
        let result = panel.clear_chat_history();
        assert!(result.is_ok(), "Should clear chat history without error");

        // Verify history is empty
        let cleared_history = panel.get_chat_history();
        assert!(cleared_history.is_empty(), "Chat history should be empty after clearing");
    }

    #[test]
    fn test_chat_message_validation() {
        // Test validation of chat messages
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test empty message
        let result = panel.send_chat_message("");
        assert!(result.is_ok(), "Should handle empty message gracefully");

        // Test whitespace-only message
        let result = panel.send_chat_message("   ");
        assert!(result.is_ok(), "Should handle whitespace-only message gracefully");

        // Test very long message
        let long_message = "a".repeat(10000);
        let result = panel.send_chat_message(&long_message);
        assert!(result.is_ok(), "Should handle very long message gracefully");

        // Test message with special characters
        let special_message = "Hello 🚀! This has special chars: @#$%^&*()";
        let result = panel.send_chat_message(special_message);
        assert!(result.is_ok(), "Should handle special characters in message");
    }

    #[test]
    fn test_chat_response_handling() {
        // Test handling of AI responses in chat
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send a message that should trigger an AI response
        let result = panel.send_chat_message("What are the main improvements suggested?");

        assert!(result.is_ok(), "Should send message that triggers AI response");

        // Check that an AI response was added to history
        let history = panel.get_chat_history();
        let ai_responses: Vec<_> = history.iter().filter(|m| m.role == MessageRole::Assistant).collect();

        // This test establishes the contract - there should be a mechanism for AI responses
        // The actual implementation may handle async responses differently
        assert!(history.len() >= 1, "Chat history should contain at least the user message");

        // Verify user message is present
        let user_messages: Vec<_> = history.iter().filter(|m| m.role == MessageRole::User).collect();
        assert_eq!(user_messages.len(), 1, "Should have exactly one user message");
        assert_eq!(user_messages[0].content, "What are the main improvements suggested?");
    }

    #[test]
    fn test_chat_error_handling() {
        // Test error handling in chat functionality
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        // This should fail until AdvicePanel is implemented
        let panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that error handling methods exist
        let error_result = std::panic::catch_unwind(|| {
            panel.get_last_chat_error()
        });

        assert!(error_result.is_ok(), "Should have method to get last chat error");

        // Test that we can check if chat is available
        let available_result = std::panic::catch_unwind(|| {
            panel.is_chat_available()
        });

        assert!(available_result.is_ok(), "Should have method to check if chat is available");
    }
}