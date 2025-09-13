use crate::config::LlmConfig;
use crate::git::GitRepo;
use log::{debug, error};
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use std::env;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct AsyncLLMCommand {
    pub result_rx: mpsc::Receiver<LLMResult>,
}

#[derive(Debug, Clone)]
pub enum LLMResult {
    Success(String),
    Error(String),
}

impl AsyncLLMCommand {
    pub fn new(config: LlmConfig, interval: u64) -> Self {
        let (result_tx, result_rx) = mpsc::channel(32);

        tokio::spawn(async move {
            let mut last_run_time: Option<Instant> = None;

            loop {
                let should_run = if let Some(last) = last_run_time {
                    last.elapsed() >= Duration::from_secs(interval)
                } else {
                    true
                };

                if should_run {
                    debug!("Running async LLM command");

                    let api_key = config
                        .api_key
                        .clone()
                        .or_else(|| env::var("OPENAI_API_KEY").ok());

                    if api_key.is_none() {
                        let error_str = "OpenAI API key not found. Please set it in the config file or as an environment variable.".to_string();
                        if result_tx.send(LLMResult::Error(error_str)).await.is_err() {
                            break;
                        }
                        tokio::time::sleep(Duration::from_secs(3600)).await; // Wait for an hour before retrying
                        continue;
                    }

                    let repo_path = match std::env::current_dir() {
                        Ok(path) => path,
                        Err(e) => {
                            error!("Failed to get current directory: {}", e);
                            break;
                        }
                    };

                    let mut git_repo = match GitRepo::new(repo_path) {
                        Ok(repo) => repo,
                        Err(e) => {
                            error!("Failed to create GitRepo: {}", e);
                            break;
                        }
                    };

                    if let Err(e) = git_repo.update() {
                        error!("Failed to update git repo: {}", e);
                        break;
                    }

                    let diff = git_repo.get_diff_string();

                    if diff.is_empty() {
                        debug!("No diff found, skipping LLM query.");
                        last_run_time = Some(Instant::now());
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }

                    let prompt = format!(
                        "You are acting in the role of a staff engineer providing a code review. \
Please provide a brief review of the following code changes. \
The review should focus on 'Maintainability' and any obvious safety bugs. \
In the maintainability part, include 0-3 actionable suggestions to enhance code maintainability. \
Don't be afraid to say that this code is okay at maintainability and not provide suggestions. \
When you provide suggestions, give a brief before and after example using the code diffs below \
to provide context and examples of what you mean. \
Each suggestion should be clear, specific, and implementable. \
Keep the response concise and focused on practical improvements.

```diff
{}
```",
                        diff
                    );

                    let mut client = OpenAIClient::builder()
                        .with_api_key(api_key.unwrap())
                        .build()
                        .unwrap();
                    let req = ChatCompletionRequest::new(
                        config.model.clone().unwrap_or("gpt-3.5-turbo".to_string()),
                        vec![chat_completion::ChatCompletionMessage {
                            role: chat_completion::MessageRole::user,
                            content: chat_completion::Content::Text(prompt),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        }],
                    );

                    match client.chat_completion(req).await {
                        Ok(response) => {
                            if let Some(choice) = response.choices.first() {
                                let content = choice.message.content.clone().unwrap_or_default();
                                if result_tx.send(LLMResult::Success(content)).await.is_err() {
                                    break;
                                }
                                debug!("Async LLM command completed successfully");
                            }
                        }
                        Err(e) => {
                            let error_str = format!("LLM command execution failed: {}", e);
                            if result_tx.send(LLMResult::Error(error_str)).await.is_err() {
                                break;
                            }
                            debug!("Async LLM command execution error: {}", e);
                        }
                    }

                    let now = Instant::now();
                    last_run_time = Some(now);
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        Self { result_rx }
    }

    pub fn try_get_result(&mut self) -> Option<LLMResult> {
        self.result_rx.try_recv().ok()
    }
}
