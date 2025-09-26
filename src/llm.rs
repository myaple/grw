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
        debug!(
            "🤖 LLM_CLIENT: Creating new client with config: {:?}",
            config
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
                        commit_message, diff_content
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::LlmConfig;

    #[test]
    fn test_llm_client_new_no_api_key() {
        let config = LlmConfig::default();
        let result = LlmClient::new(config);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "No API key provided");
    }

    #[test]
    fn test_llm_client_new_with_api_key() {
        let mut config = LlmConfig::default();
        config.api_key = Some("test-key".to_string());
        let result = LlmClient::new(config);
        // This might fail due to network issues, but should at least get past the API key check
        // In a real test, you'd mock the HTTP client
    }
}
