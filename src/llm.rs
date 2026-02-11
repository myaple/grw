use crate::config::LlmConfig;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionMessage, ChatCompletionRequest};
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmAdviceResult {
    pub id: String,
    pub content: String,
    pub execution_time: std::time::Duration,
    pub has_error: bool,
}

#[derive(Debug, Clone)]
pub struct LlmClient {
    client: Arc<Mutex<OpenAIClient>>,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self, String> {
        // Create a redacted clone for logging to ensure secrets are never leaked
        // even if the Debug implementation of LlmConfig changes.
        let mut log_config = config.clone();
        if log_config.api_key.is_some() {
            log_config.api_key = Some("REDACTED".to_string());
        }

        debug!(
            "🤖 LLM_CLIENT: Creating new client with config: {:?}",
            log_config
        );

        let api_key = config
            .api_key
            .clone()
            .or_else(|| env::var("OPENAI_API_KEY").ok())
            .ok_or_else(|| {
                "OpenAI API key not found. Please set it in the config file or as an environment variable."
                    .to_string()
            })?;

        let mut builder = OpenAIClient::builder().with_api_key(api_key);

        if let Some(base_url) = &config.base_url {
            builder = builder.with_endpoint(base_url);
        }

        let client = Arc::new(Mutex::new(builder.build().map_err(|e| e.to_string())?));

        Ok(Self { client, config })
    }

    pub async fn get_llm_summary(
        &self,
        commit_message: String,
        diff_content: String,
    ) -> Result<LlmAdviceResult, String> {
        let start_time = tokio::time::Instant::now();

        // Truncate diff content if needed based on config
        // Convert tokens to characters using 3 chars per token ratio
        let max_tokens = self.config.get_max_tokens();
        let max_chars = max_tokens * 3;
        let truncated_diff = if diff_content.len() > max_chars {
            let truncated = diff_content.chars().take(max_chars).collect::<String>();
            format!("{}\n\n[... diff truncated for brevity ...]", truncated)
        } else {
            diff_content.clone()
        };

        // Build the prompt for commit summary
        let messages = vec![
            ChatCompletionMessage {
                role: chat_completion::MessageRole::system,
                content: chat_completion::Content::Text(
                    "You are an expert at summarizing code changes. Generate a concise, clear summary of the following commit changes. Focus on the key changes and their impact.".to_string(),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatCompletionMessage {
                role: chat_completion::MessageRole::user,
                content: chat_completion::Content::Text(
                    format!(
                        "Commit message: {}\n\nPlease summarize these changes:\n\n{}",
                        commit_message, truncated_diff
                    ),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let result = self
            .make_llm_request(self.config.get_summary_model(), messages)
            .await;
        let execution_time = start_time.elapsed();

        match result {
            Ok(content) => Ok(LlmAdviceResult {
                id: uuid::Uuid::new_v4().to_string(),
                content,
                execution_time,
                has_error: false,
            }),
            Err(error) => Ok(LlmAdviceResult {
                id: uuid::Uuid::new_v4().to_string(),
                content: format!("❌ Failed to generate summary: {}", error),
                execution_time,
                has_error: true,
            }),
        }
    }

    /// Get the maximum number of tokens to send to LLM, with a sensible default
    pub fn get_max_tokens(&self) -> usize {
        self.config.get_max_tokens()
    }

    async fn make_llm_request(
        &self,
        model: String,
        messages: Vec<ChatCompletionMessage>,
    ) -> Result<String, String> {
        debug!(
            "🤖 LLM_CLIENT: Making request to model: {} ({} messages)",
            model,
            messages.len()
        );

        let req = ChatCompletionRequest::new(model, messages);

        let mut client = self.client.lock().await;

        match client.chat_completion(req).await {
            Ok(response) => {
                if let Some(choice) = response.choices.first() {
                    let content = choice.message.content.clone().unwrap_or_default();
                    Ok(content)
                } else {
                    Err("No response from LLM".to_string())
                }
            }
            Err(e) => Err(format!("LLM command execution failed: {e}")),
        }
    }

    /// Send a chat follow-up message for advice context
    pub async fn send_chat_followup(
        &self,
        question: String,
        conversation_history: Vec<crate::pane::ChatMessageData>,
    ) -> Result<crate::pane::ChatMessageData, String> {
        let start_time = tokio::time::Instant::now();
        debug!("🤖 LLM_CLIENT: Processing chat follow-up");

        // Build conversation context
        let mut context_messages = vec![ChatCompletionMessage {
            role: chat_completion::MessageRole::system,
            content: chat_completion::Content::Text(
                "You are an expert software engineer helping with code improvements. \
                    The user is asking about specific code changes and improvements. \
                    Be helpful, specific, and provide practical advice. \
                    Keep your responses concise but thorough."
                    .to_string(),
            ),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }];

        // Add conversation history (which already contains the initial message with diff context)
        for msg in conversation_history {
            let role = match msg.role {
                crate::pane::MessageRole::User => chat_completion::MessageRole::user,
                crate::pane::MessageRole::Assistant => chat_completion::MessageRole::assistant,
                crate::pane::MessageRole::System => chat_completion::MessageRole::system,
            };
            context_messages.push(ChatCompletionMessage {
                role,
                content: chat_completion::Content::Text(msg.content),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            });
        }

        // Add the current question
        context_messages.push(ChatCompletionMessage {
            role: chat_completion::MessageRole::user,
            content: chat_completion::Content::Text(question),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });

        debug!(
            "🤖 LLM_CLIENT: About to make HTTP request to LLM API with {} messages",
            context_messages.len()
        );
        let result = self
            .make_llm_request(self.config.get_advice_model(), context_messages)
            .await;

        let execution_time = start_time.elapsed();
        debug!(
            "🤖 LLM_CLIENT: Chat follow-up completed in {:?}",
            execution_time
        );

        match result {
            Ok(content) => Ok(crate::pane::ChatMessageData {
                id: uuid::Uuid::new_v4().to_string(),
                role: crate::pane::MessageRole::Assistant,
                content,
                timestamp: std::time::SystemTime::now(),
            }),
            Err(error) => {
                debug!("🤖 LLM_CLIENT: Failed to process chat: {}", error);
                Err(format!("Failed to process chat: {}", error))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    #[test]
    fn test_llm_client_new_no_api_key() {
        // Temporarily remove the environment variable for this test
        let original_key = std::env::var("OPENAI_API_KEY").ok();
        unsafe {
            std::env::remove_var("OPENAI_API_KEY");
        }

        let config = LlmConfig::default();
        let result = LlmClient::new(config);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OpenAI API key not found"));

        // Restore the original environment variable
        if let Some(key) = original_key {
            unsafe {
                std::env::set_var("OPENAI_API_KEY", key);
            }
        }
    }

    #[test]
    fn test_llm_client_new_with_api_key() {
        let config = LlmConfig {
            api_key: Some("test-key".to_string()),
            ..Default::default()
        };
        let _result = LlmClient::new(config);
        // This might fail due to network issues, but should at least get past the API key check
        // In a real test, you'd mock the HTTP client
    }
}
