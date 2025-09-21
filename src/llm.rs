use crate::config::LlmConfig;
use crate::git::GitRepo;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use std::env;
use std::fs;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc, watch};

const DEFAULT_MODEL: &str = "gpt-5-mini";

#[derive(Clone)]
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
        history: Vec<chat_completion::ChatCompletionMessage>,
    ) -> Result<String, String> {
        let model = self
            .config
            .model
            .clone()
            .unwrap_or_else(|| DEFAULT_MODEL.to_string());

        let req = ChatCompletionRequest::new(model, history);

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
    pub fn new(llm_client: LlmClient) -> Self {
        let (result_tx, result_rx) = mpsc::channel(32);
        let (git_repo_tx, mut git_repo_rx) = watch::channel(None);
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(()) = refresh_rx.recv() => {
                        debug!("Running async LLM command");

                        if git_repo_rx.borrow().is_none() && git_repo_rx.changed().await.is_err() {
                            break;
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

                        let prompt_template = llm_client.config.prompt.clone().unwrap_or_else(|| {
                            "You are acting in the role of a staff engineer providing a code review. \
                Please provide a brief review of the following code changes. \
                The review should focus on 'Maintainability' and any obvious safety bugs. \
                In the maintainability part, include 0-3 actionable suggestions to enhance code maintainability. \
                Don't be afraid to say that this code is okay at maintainability and not provide suggestions. \
                When you provide suggestions, give a brief before and after example using the code diffs below \
                to provide context and examples of what you mean. \
                Each suggestion should be clear, specific, and implementable. \
                Keep the response concise and focused on practical improvements.".to_string()
                        });

                        let prompt = format!(
                            "{claude_instructions}\n\n{prompt_template}\n\n```diff\n{diff}\n```"
                        );

                        let history = vec![chat_completion::ChatCompletionMessage {
                            role: chat_completion::MessageRole::user,
                            content: chat_completion::Content::Text(prompt),
                            name: None,
                            tool_calls: None,
                            tool_call_id: None,
                        }];

                        match llm_client.get_llm_advice(history).await {
                            Ok(content) => {
                                if result_tx.send(LLMResult::Success(content)).await.is_err() {
                                    break;
                                }
                                debug!("Async LLM command completed successfully");
                            }
                            Err(e) => {
                                let error_str = format!("LLM command execution failed: {e}");
                                if result_tx.send(LLMResult::Error(error_str)).await.is_err() {
                                    break;
                                }
                                debug!("Async LLM command execution error: {e}");
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
