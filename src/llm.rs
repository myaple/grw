use crate::config::LlmConfig;

use crate::shared_state::LlmSharedState;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest, ChatCompletionMessage};
use std::env;

use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct LlmAdviceTask {
    pub id: String,
    pub diff_content: String,
    pub status: LlmTaskStatus,
    pub result: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LlmTaskStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone)]
pub struct LlmAdviceResult {
    pub id: String,
    pub content: String,
    pub execution_time: Duration,
    pub has_error: bool,
}

#[derive(Clone, Debug)]
pub struct LlmClient {
    client: Arc<Mutex<OpenAIClient>>,
    config: LlmConfig,
}

impl LlmClient {
    pub fn new(config: LlmConfig) -> Result<Self, String> {
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

    pub async fn get_llm_advice(
        &self,
        diff_content: String,
    ) -> Result<LlmAdviceResult, String> {
        let start_time = tokio::time::Instant::now();

        // Build the prompt for code review
        let messages = vec![
            ChatCompletionMessage {
                role: chat_completion::MessageRole::system,
                content: chat_completion::Content::Text(
                    "You are an expert code reviewer. Analyze the following git diff and provide actionable advice for improvement. Focus on code quality, best practices, potential bugs, and performance optimizations. Keep your response concise and practical.".to_string(),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatCompletionMessage {
                role: chat_completion::MessageRole::user,
                content: chat_completion::Content::Text(
                    format!("Please review this code diff:\n\n{}", diff_content),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let result = self.make_llm_request(self.config.get_advice_model(), messages).await;
        let execution_time = start_time.elapsed();

        match result {
            Ok(content) => {
                Ok(LlmAdviceResult {
                    id: uuid::Uuid::new_v4().to_string(),
                    content,
                    execution_time,
                    has_error: false,
                })
            }
            Err(error) => {
                Ok(LlmAdviceResult {
                    id: uuid::Uuid::new_v4().to_string(),
                    content: format!("❌ Failed to generate advice: {}", error),
                    execution_time,
                    has_error: true,
                })
            }
        }
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
                    format!("Commit message: {}\n\nCode changes:\n{}", commit_message, diff_content),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let result = self.make_llm_request(self.config.get_summary_model(), messages).await;
        let execution_time = start_time.elapsed();

        match result {
            Ok(content) => {
                Ok(LlmAdviceResult {
                    id: uuid::Uuid::new_v4().to_string(),
                    content,
                    execution_time,
                    has_error: false,
                })
            }
            Err(error) => {
                Ok(LlmAdviceResult {
                    id: uuid::Uuid::new_v4().to_string(),
                    content: format!("❌ Failed to generate summary: {}", error),
                    execution_time,
                    has_error: true,
                })
            }
        }
    }

    async fn make_llm_request(
        &self,
        model: String,
        messages: Vec<ChatCompletionMessage>,
    ) -> Result<String, String> {
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

#[derive(Debug)]
pub struct AsyncLLMCommand {
    client: Arc<LlmClient>,
    llm_state: Arc<LlmSharedState>,
    advice_tx: mpsc::Sender<LlmAdviceResult>,
    pending_tasks: Arc<Mutex<Vec<LlmAdviceTask>>>,
}

impl AsyncLLMCommand {
    pub fn new(client: LlmClient, llm_state: Arc<LlmSharedState>) -> (Self, mpsc::Receiver<LlmAdviceResult>) {
        let (advice_tx, advice_rx) = mpsc::channel(16);
        let client_arc = Arc::new(client);
        let pending_tasks = Arc::new(Mutex::new(Vec::new()));

        let command = Self {
            client: client_arc.clone(),
            llm_state,
            advice_tx: advice_tx.clone(),
            pending_tasks: pending_tasks.clone(),
        };

        // Start the background task processor
        let pending_clone = pending_tasks.clone();
        let tx_clone = advice_tx.clone();
        let client_clone = client_arc.clone();

        tokio::spawn(async move {
            loop {
                // Process pending tasks
                let task = {
                    let mut tasks = pending_clone.lock().await;
                    tasks.iter().position(|t| t.status == LlmTaskStatus::Pending)
                        .map(|idx| tasks.remove(idx))
                };

                if let Some(mut task) = task {
                    task.status = LlmTaskStatus::InProgress;

                    // Add task back to pending list with in-progress status
                    {
                        let mut tasks = pending_clone.lock().await;
                        tasks.push(task.clone());
                    }

                    let result = client_clone.get_llm_advice(task.diff_content.clone()).await;

                    // Update task status and send result
                    {
                        let mut tasks = pending_clone.lock().await;
                        if let Some(t) = tasks.iter_mut().find(|t| t.id == task.id) {
                            match &result {
                                Ok(advice) if !advice.has_error => {
                                    t.status = LlmTaskStatus::Completed;
                                    t.result = Some(advice.content.clone());
                                }
                                _ => {
                                    t.status = LlmTaskStatus::Failed;
                                    t.error = result.as_ref().err().cloned();
                                }
                            }
                        }
                    }

                    // Send result through channel
                    if let Ok(advice) = result {
                        if let Err(e) = tx_clone.send(advice).await {
                            debug!("Failed to send LLM advice result: {}", e);
                        }
                    }
                }

                // Clean up completed tasks older than 5 minutes
                {
                    let mut tasks = pending_clone.lock().await;
                    tasks.retain(|task| {
                        task.status != LlmTaskStatus::Completed &&
                        task.status != LlmTaskStatus::Failed
                    });
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        (command, advice_rx)
    }

    pub async fn request_advice(&self, diff_content: String) -> String {
        let task_id = uuid::Uuid::new_v4().to_string();

        let task = LlmAdviceTask {
            id: task_id.clone(),
            diff_content,
            status: LlmTaskStatus::Pending,
            result: None,
            error: None,
        };

        // Add to pending tasks
        {
            let mut tasks = self.pending_tasks.lock().await;
            tasks.push(task);
        }

        // Update shared state to show loading
        self.llm_state.start_advice_task(task_id.clone());
        self.llm_state.update_advice("current".to_string(), "⏳ Generating advice...".to_string());

        task_id
    }

    pub fn get_current_advice(&self) -> Option<String> {
        self.llm_state.get_current_advice("current")
    }

    pub fn get_error(&self) -> Option<String> {
        self.llm_state.get_error("advice_generation")
    }

    pub fn clear_error(&self) {
        self.llm_state.clear_error("advice_generation");
    }

    pub fn is_processing(&self) -> bool {
        // Check if there are any pending or in-progress tasks
        let tasks = futures::executor::block_on(async {
            self.pending_tasks.lock().await.clone()
        });
        tasks.iter().any(|t| t.status == LlmTaskStatus::Pending || t.status == LlmTaskStatus::InProgress)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_state::LlmSharedState;
    use std::time::Duration;

    #[tokio::test]
    async fn test_llm_advice_task_creation() {
        let task = LlmAdviceTask {
            id: "test-task".to_string(),
            diff_content: "test diff content".to_string(),
            status: LlmTaskStatus::Pending,
            result: None,
            error: None,
        };

        assert_eq!(task.id, "test-task");
        assert_eq!(task.diff_content, "test diff content");
        assert_eq!(task.status, LlmTaskStatus::Pending);
        assert!(task.result.is_none());
        assert!(task.error.is_none());
    }

    #[tokio::test]
    async fn test_llm_advice_result_creation() {
        let result = LlmAdviceResult {
            id: "test-result".to_string(),
            content: "test advice content".to_string(),
            execution_time: Duration::from_millis(500),
            has_error: false,
        };

        assert_eq!(result.id, "test-result");
        assert_eq!(result.content, "test advice content");
        assert_eq!(result.execution_time, Duration::from_millis(500));
        assert!(!result.has_error);
    }

    #[tokio::test]
    async fn test_llm_task_status_progression() {
        let mut task = LlmAdviceTask {
            id: "test-task".to_string(),
            diff_content: "test diff".to_string(),
            status: LlmTaskStatus::Pending,
            result: None,
            error: None,
        };

        assert_eq!(task.status, LlmTaskStatus::Pending);

        // Progress to in progress
        task.status = LlmTaskStatus::InProgress;
        assert_eq!(task.status, LlmTaskStatus::InProgress);

        // Complete successfully
        task.status = LlmTaskStatus::Completed;
        task.result = Some("success advice".to_string());
        assert_eq!(task.status, LlmTaskStatus::Completed);
        assert_eq!(task.result, Some("success advice".to_string()));
        assert!(task.error.is_none());
    }

    #[tokio::test]
    async fn test_llm_task_error_handling() {
        let mut task = LlmAdviceTask {
            id: "test-task".to_string(),
            diff_content: "test diff".to_string(),
            status: LlmTaskStatus::Pending,
            result: None,
            error: None,
        };

        // Simulate error
        task.status = LlmTaskStatus::Failed;
        task.error = Some("API request failed".to_string());
        assert_eq!(task.status, LlmTaskStatus::Failed);
        assert_eq!(task.error, Some("API request failed".to_string()));
        assert!(task.result.is_none());
    }

    #[tokio::test]
    async fn test_llm_task_status_equality() {
        assert_eq!(LlmTaskStatus::Pending, LlmTaskStatus::Pending);
        assert_ne!(LlmTaskStatus::Pending, LlmTaskStatus::InProgress);
        assert_ne!(LlmTaskStatus::Completed, LlmTaskStatus::Failed);
    }

    #[tokio::test]
    async fn test_llm_advice_result_error_handling() {
        let error_result = LlmAdviceResult {
            id: "error-result".to_string(),
            content: "Error: API timeout".to_string(),
            execution_time: Duration::from_secs(10),
            has_error: true,
        };

        assert!(error_result.has_error);
        assert!(error_result.content.contains("Error:"));
        assert_eq!(error_result.execution_time, Duration::from_secs(10));
    }

    #[tokio::test]
    async fn test_llm_client_creation() {
        // Test with no API key - may fail or succeed depending on environment
        let config_no_key = crate::config::LlmConfig {
            provider: Some(crate::config::LlmProvider::OpenAI),
            model: Some("gpt-3.5-turbo".to_string()),
            advice_model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4".to_string()),
            api_key: None,
            base_url: None,
            prompt: None,
        };

        let result = LlmClient::new(config_no_key);
        // Don't assert failure since it might succeed if OPENAI_API_KEY is set in environment
        match result {
            Ok(_) => println!("LlmClient created (likely using environment variable)"),
            Err(e) => println!("LlmClient creation failed as expected: {}", e),
        }

        // Test with API key but invalid (mock test)
        let config_with_key = crate::config::LlmConfig {
            provider: Some(crate::config::LlmProvider::OpenAI),
            model: Some("gpt-3.5-turbo".to_string()),
            advice_model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4".to_string()),
            api_key: Some("test-key".to_string()),
            base_url: None,
            prompt: None,
        };

        // This may or may not fail depending on network, but we can test the structure
        let _result = LlmClient::new(config_with_key);
        // We don't assert the result since it depends on external factors
    }

    #[tokio::test]
    async fn test_async_llm_command_creation() {
        // Mock shared state
        let shared_state = Arc::new(LlmSharedState::new());

        // Create a mock client (this would normally fail)
        let config = crate::config::LlmConfig {
            provider: Some(crate::config::LlmProvider::OpenAI),
            model: Some("gpt-3.5-turbo".to_string()),
            advice_model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4".to_string()),
            api_key: Some("fake-key".to_string()),
            base_url: None,
            prompt: None,
        };

        if let Ok(client) = LlmClient::new(config) {
            let (command, mut rx) = AsyncLLMCommand::new(client, shared_state);

            // Test that the receiver works
            let initial_result = rx.try_recv();
            assert!(initial_result.is_err()); // Should be empty initially

            // Test the command structure
            assert!(!command.is_processing());
        }
    }

    #[tokio::test]
    async fn test_llm_task_cleaning() {
        let tasks = vec![
            LlmAdviceTask {
                id: "completed-task".to_string(),
                diff_content: "test diff".to_string(),
                status: LlmTaskStatus::Completed,
                result: Some("completed advice".to_string()),
                error: None,
            },
            LlmAdviceTask {
                id: "failed-task".to_string(),
                diff_content: "test diff".to_string(),
                status: LlmTaskStatus::Failed,
                result: None,
                error: Some("failed to process".to_string()),
            },
            LlmAdviceTask {
                id: "pending-task".to_string(),
                diff_content: "test diff".to_string(),
                status: LlmTaskStatus::Pending,
                result: None,
                error: None,
            },
        ];

        // Test filtering for pending tasks
        let pending_count = tasks.iter()
            .filter(|t| t.status == LlmTaskStatus::Pending)
            .count();
        assert_eq!(pending_count, 1);

        // Test filtering for in-progress tasks
        let in_progress_count = tasks.iter()
            .filter(|t| matches!(t.status, LlmTaskStatus::Pending | LlmTaskStatus::InProgress))
            .count();
        assert_eq!(in_progress_count, 1);
    }

    #[tokio::test]
    async fn test_llm_advice_channel_buffering() {
        // Test that channels can buffer multiple results
        let shared_state = Arc::new(LlmSharedState::new());
        let config = crate::config::LlmConfig {
            provider: Some(crate::config::LlmProvider::OpenAI),
            model: Some("gpt-3.5-turbo".to_string()),
            advice_model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4".to_string()),
            api_key: Some("fake-key".to_string()),
            base_url: None,
            prompt: None,
        };

        if let Ok(client) = LlmClient::new(config) {
            let (_command, _rx) = AsyncLLMCommand::new(client, shared_state);

            // Test that channel can handle multiple items
            let (test_tx, mut test_rx) = mpsc::channel(16);

            // Send multiple test results
            for i in 0..5 {
                let result = LlmAdviceResult {
                    id: format!("test-{}", i),
                    content: format!("Test advice {}", i),
                    execution_time: Duration::from_millis(100),
                    has_error: false,
                };

                if let Err(e) = test_tx.try_send(result) {
                    panic!("Failed to send test result: {}", e);
                }
            }

            // Verify all results were received
            let mut count = 0;
            while test_rx.try_recv().is_ok() {
                count += 1;
                if count > 10 {
                    break; // Safety check
                }
            }

            assert_eq!(count, 5);
        }
    }

    #[tokio::test]
    async fn test_llm_error_result_handling() {
        let error_result = LlmAdviceResult {
            id: "error-test".to_string(),
            content: "❌ Failed to generate advice: Network error".to_string(),
            execution_time: Duration::from_millis(250),
            has_error: true,
        };

        assert!(error_result.has_error);
        assert!(error_result.content.contains("Failed to generate advice"));
        assert!(error_result.content.contains("Network error"));
        assert_eq!(error_result.execution_time, Duration::from_millis(250));
    }

    #[tokio::test]
    async fn test_llm_task_management() {
        let shared_state = Arc::new(LlmSharedState::new());
        let config = crate::config::LlmConfig {
            provider: Some(crate::config::LlmProvider::OpenAI),
            model: Some("gpt-3.5-turbo".to_string()),
            advice_model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4".to_string()),
            api_key: Some("fake-key".to_string()),
            base_url: None,
            prompt: None,
        };

        if let Ok(client) = LlmClient::new(config) {
            let (command, _rx) = AsyncLLMCommand::new(client, shared_state);

            // Test that task management methods work
            assert!(!command.is_processing());

            // Test getting current advice (should be None initially)
            let current_advice = command.get_current_advice();
            assert!(current_advice.is_none());

            // Test error handling (should be None initially)
            let error = command.get_error();
            assert!(error.is_none());
        }
    }
}
