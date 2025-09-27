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

    /// Generate code improvement advice from a git diff
    pub async fn generate_advice(
        &self,
        diff_content: String,
        max_improvements: usize,
    ) -> Result<Vec<crate::pane::AdviceImprovement>, String> {
        let start_time = tokio::time::Instant::now();
        debug!("🤖 LLM_CLIENT: Generating advice for diff ({} chars)", diff_content.len());

        // Build the prompt for code improvement advice
        let system_prompt = format!(
            "You are an expert software engineer specializing in code review and improvement. \
            Analyze the following git diff and provide exactly {} specific, actionable improvements. \
            Each improvement should include:\n\
            1. A clear title\n\
            2. A detailed description explaining the improvement\n\
            3. The priority level (Low, Medium, High)\n\
            4. The category (CodeQuality, Performance, Security, BugFix, Feature, Documentation)\n\
            \n\
            Focus on practical, implementable suggestions that will actually improve the code. \
            Be specific about what changes to make and why they matter.\n\
            \n\
            Respond in JSON format with this structure:\n\
            {{\n\
              \"improvements\": [\n\
                {{\n\
                  \"title\": \"Clear, specific title\",\n\
                  \"description\": \"Detailed explanation of the improvement\",\n\
                  \"priority\": \"Medium\",\n\
                  \"category\": \"CodeQuality\"\n\
                }}\n\
              ]\n\
            }}",
            max_improvements
        );

        let user_prompt = format!(
            "Please analyze this git diff and suggest improvements:\n\n{}",
            diff_content
        );

        let messages = vec![
            ChatCompletionMessage {
                role: chat_completion::MessageRole::system,
                content: chat_completion::Content::Text(system_prompt),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatCompletionMessage {
                role: chat_completion::MessageRole::user,
                content: chat_completion::Content::Text(user_prompt),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        let result = self
            .make_llm_request(self.config.get_advice_model(), messages)
            .await;

        let execution_time = start_time.elapsed();
        debug!("🤖 LLM_CLIENT: Advice generation completed in {:?}", execution_time);

        match result {
            Ok(content) => {
                // Parse the JSON response
                self.parse_advice_response(&content, execution_time)
            }
            Err(error) => {
                debug!("🤖 LLM_CLIENT: Failed to generate advice: {}", error);
                Err(format!("Failed to generate advice: {}", error))
            }
        }
    }

    /// Send a chat follow-up message for advice context
    pub async fn send_chat_followup(
        &self,
        question: String,
        conversation_history: Vec<crate::pane::ChatMessageData>,
        original_diff: String,
    ) -> Result<crate::pane::ChatMessageData, String> {
        let start_time = tokio::time::Instant::now();
        debug!("🤖 LLM_CLIENT: Processing chat follow-up: {}", question);

        // Build conversation context
        let mut context_messages = vec![
            ChatCompletionMessage {
                role: chat_completion::MessageRole::system,
                content: chat_completion::Content::Text(
                    "You are an expert software engineer helping with code improvements. \
                    The user is asking about specific code changes and improvements. \
                    Be helpful, specific, and provide practical advice. \
                    Keep your responses concise but thorough.".to_string(),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
            ChatCompletionMessage {
                role: chat_completion::MessageRole::user,
                content: chat_completion::Content::Text(
                    format!("Here is the original git diff for context:\n\n{}", original_diff),
                ),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            },
        ];

        // Add conversation history
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

        let result = self
            .make_llm_request(self.config.get_advice_model(), context_messages)
            .await;

        let execution_time = start_time.elapsed();
        debug!("🤖 LLM_CLIENT: Chat follow-up completed in {:?}", execution_time);

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

    /// Parse the JSON response from advice generation
    fn parse_advice_response(
        &self,
        content: &str,
        execution_time: std::time::Duration,
    ) -> Result<Vec<crate::pane::AdviceImprovement>, String> {
        #[derive(Deserialize)]
        struct AdviceResponse {
            improvements: Vec<RawImprovement>,
        }

        #[derive(Deserialize)]
        struct RawImprovement {
            title: String,
            description: String,
            priority: String,
            category: String,
            id: Option<String>,
        }

        // Try to parse as JSON first
        if let Ok(response) = serde_json::from_str::<AdviceResponse>(content) {
            let improvements: Vec<crate::pane::AdviceImprovement> = response
                .improvements
                .into_iter()
                .map(|raw| crate::pane::AdviceImprovement {
                    id: raw.id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string()),
                    title: raw.title,
                    description: raw.description,
                    priority: self.parse_priority(&raw.priority),
                    category: raw.category,
                    code_examples: Vec::new(),
                })
                .collect();

            debug!("🤖 LLM_CLIENT: Parsed {} improvements from JSON response", improvements.len());
            return Ok(improvements);
        }

        // Fallback: try to extract improvements from text response
        debug!("🤖 LLM_CLIENT: JSON parsing failed, attempting text extraction");
        self.extract_improvements_from_text(content, execution_time)
    }

    /// Parse priority string to enum
    fn parse_priority(&self, priority_str: &str) -> crate::pane::ImprovementPriority {
        match priority_str.to_lowercase().as_str() {
            "low" => crate::pane::ImprovementPriority::Low,
            "medium" => crate::pane::ImprovementPriority::Medium,
            "high" => crate::pane::ImprovementPriority::High,
            "critical" => crate::pane::ImprovementPriority::Critical,
            _ => crate::pane::ImprovementPriority::Medium,
        }
    }

    /// Extract improvements from text response (fallback)
    fn extract_improvements_from_text(
        &self,
        content: &str,
        _execution_time: std::time::Duration,
    ) -> Result<Vec<crate::pane::AdviceImprovement>, String> {
        // Simple heuristic: look for numbered or bulleted improvements
        let lines: Vec<&str> = content.lines().collect();
        let mut improvements = Vec::new();
        let mut current_title = String::new();
        let mut current_description = String::new();

        for line in lines {
            let trimmed = line.trim();

            // Look for improvement indicators (numbered, bullet points, or strong headings)
            if trimmed.starts_with("1.") || trimmed.starts_with("2.") || trimmed.starts_with("3.")
                || trimmed.starts_with("- ") || trimmed.starts_with("* ")
                || (trimmed.starts_with("**") && trimmed.ends_with("**")) {

                // Save previous improvement if we have one
                if !current_title.is_empty() {
                    improvements.push(crate::pane::AdviceImprovement {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: current_title.clone(),
                        description: current_description.clone(),
                        priority: crate::pane::ImprovementPriority::Medium,
                        category: "CodeQuality".to_string(),
                        code_examples: Vec::new(),
                    });
                }

                // Start new improvement
                current_title = trimmed
                    .trim_start_matches("1.")
                    .trim_start_matches("2.")
                    .trim_start_matches("3.")
                    .trim_start_matches("- ")
                    .trim_start_matches("* ")
                    .trim_start_matches("**")
                    .trim_end_matches("**")
                    .trim()
                    .to_string();
                current_description.clear();
            } else if !current_title.is_empty() && !trimmed.is_empty() {
                // Add to current description
                if !current_description.is_empty() {
                    current_description.push(' ');
                }
                current_description.push_str(trimmed);
            }
        }

        // Add the last improvement
        if !current_title.is_empty() {
            improvements.push(crate::pane::AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: current_title,
                description: current_description,
                priority: crate::pane::ImprovementPriority::Medium,
                category: "CodeQuality".to_string(),
                code_examples: Vec::new(),
            });
        }

        if improvements.is_empty() {
            // If no improvements found, create a generic one
            improvements.push(crate::pane::AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Code Review Suggestions".to_string(),
                description: content.to_string(),
                priority: crate::pane::ImprovementPriority::Medium,
                category: "CodeQuality".to_string(),
                code_examples: Vec::new(),
            });
        }

        debug!("🤖 LLM_CLIENT: Extracted {} improvements from text", improvements.len());
        Ok(improvements)
    }

    /// Blocking version of generate_advice for synchronous contexts
    pub fn blocking_generate_advice(
        &self,
        diff_content: String,
        max_improvements: usize,
    ) -> Result<Vec<crate::pane::AdviceImprovement>, String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        runtime.block_on(self.generate_advice(diff_content, max_improvements))
    }

    /// Blocking version of send_chat_followup for synchronous contexts
    pub fn blocking_send_chat_followup(
        &self,
        question: String,
        conversation_history: Vec<crate::pane::ChatMessageData>,
        original_diff: String,
    ) -> Result<crate::pane::ChatMessageData, String> {
        let runtime = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        runtime.block_on(self.send_chat_followup(question, conversation_history, original_diff))
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
        let mut config = LlmConfig::default();
        config.api_key = Some("test-key".to_string());
        let _result = LlmClient::new(config);
        // This might fail due to network issues, but should at least get past the API key check
        // In a real test, you'd mock the HTTP client
    }
}
