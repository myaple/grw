use crate::config::LlmConfig;
use crate::git::GitRepo;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use std::env;
use std::fs;
use tokio::sync::{mpsc, watch};

#[derive(Debug)]
pub struct AsyncLLMCommand {
    pub result_rx: mpsc::Receiver<LLMResult>,
    pub git_repo_tx: watch::Sender<Option<GitRepo>>,
    refresh_tx: mpsc::Sender<()>,
}

#[derive(Debug, Clone)]
pub enum LLMResult {
    Success(String),
    Error(String),
    Noop,
}

impl AsyncLLMCommand {
    pub fn new(config: LlmConfig) -> Self {
        let (result_tx, result_rx) = mpsc::channel(32);
        let (git_repo_tx, mut git_repo_rx) = watch::channel(None);
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(()) = refresh_rx.recv() => {
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
                            continue;
                        }

                        if git_repo_rx.borrow().is_none() {
                            if git_repo_rx.changed().await.is_err() {
                                break;
                            }
                        }

                        let git_repo = git_repo_rx.borrow().clone();

                        if git_repo.is_none() {
                            continue;
                        }
                        let git_repo: GitRepo = git_repo.unwrap();

                        let diff = git_repo.get_diff_string();

                        if diff.is_empty() {
                            debug!("No diff found, skipping LLM query.");
                            if result_tx.send(LLMResult::Noop).await.is_err() {
                                break;
                            }
                            continue;
                        }

                        let claude_instructions =
                            fs::read_to_string("CLAUDE.md").unwrap_or_default();

                        let prompt = format!(
                            "{}\n\nYou are acting in the role of a staff engineer providing a code review. \
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
                            claude_instructions, diff
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
                    },
                    else => break,
                }
            }
        });

        Self {
            result_rx,
            git_repo_tx,
            refresh_tx,
        }
    }

    pub fn refresh(&self) {
        debug!("LLM refresh requested");
        let _ = self.refresh_tx.try_send(());
    }

    pub fn try_get_result(&mut self) -> Option<LLMResult> {
        self.result_rx.try_recv().ok()
    }
}

pub async fn get_llm_advice(
    history: Vec<chat_completion::ChatCompletionMessage>,
) -> Result<String, String> {
    let api_key = env::var("OPENAI_API_KEY");

    if api_key.is_err() {
        return Err(
            "OpenAI API key not found. Please set it as an environment variable.".to_string(),
        );
    }

    let mut client = OpenAIClient::builder()
        .with_api_key(api_key.unwrap())
        .build()
        .map_err(|e| e.to_string())?;

    let req = ChatCompletionRequest::new("gpt-3.5-turbo".to_string(), history);

    match client.chat_completion(req).await {
        Ok(response) => {
            if let Some(choice) = response.choices.first() {
                let content = choice.message.content.clone().unwrap_or_default();
                Ok(content)
            } else {
                Err("No response from LLM".to_string())
            }
        }
        Err(e) => Err(format!("LLM command execution failed: {}", e)),
    }
}
