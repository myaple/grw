use crate::config::LlmConfig;
use crate::git::GitRepo;
use crate::shared_state::LlmSharedState;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use std::env;
use std::fs;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, watch};



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
        let model = self.config.get_advice_model();
        self.make_llm_request(model, history).await
    }

    pub async fn get_llm_summary(
        &self,
        history: Vec<chat_completion::ChatCompletionMessage>,
    ) -> Result<String, String> {
        let model = self.config.get_summary_model();
        self.make_llm_request(model, history).await
    }

    async fn make_llm_request(
        &self,
        model: String,
        history: Vec<chat_completion::ChatCompletionMessage>,
    ) -> Result<String, String> {
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
    pub git_repo_tx: watch::Sender<Option<GitRepo>>,
    refresh_tx: mpsc::Sender<()>,
    llm_state: Arc<LlmSharedState>,
}

#[derive(Debug, Clone)]
pub enum LLMResult {
    Success(String),
    Error(String),
    Noop,
}

impl AsyncLLMCommand {
    pub fn new(llm_client: LlmClient, llm_state: Arc<LlmSharedState>) -> Self {
        let (git_repo_tx, mut git_repo_rx) = watch::channel(None);
        let (refresh_tx, mut refresh_rx) = mpsc::channel(1);
        let llm_state_clone = Arc::clone(&llm_state);

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
                            // Store noop result in shared state
                            llm_state_clone.update_advice("current".to_string(), "No changes to review".to_string());
                            continue;
                        }

                        // Generate a task ID for tracking this advice generation
                        let task_id = format!("advice_{}", std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis());
                        
                        // Start tracking the advice task
                        llm_state_clone.start_advice_task(task_id.clone());

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
                                // Store advice result in shared state
                                llm_state_clone.update_advice("current".to_string(), content.clone());
                                llm_state_clone.cache_advice(task_id.clone(), content);
                                llm_state_clone.complete_advice_task(&task_id);
                                debug!("Async LLM command completed successfully");
                            }
                            Err(e) => {
                                let error_str = format!("LLM command execution failed: {e}");
                                // Store error in shared state
                                llm_state_clone.set_error("advice_generation".to_string(), error_str.clone());
                                llm_state_clone.complete_advice_task(&task_id);
                                debug!("Async LLM command execution error: {e}");
                            }
                        }
                    },
                    else => break,
                }
            }
        });

        Self {
            git_repo_tx,
            refresh_tx,
            llm_state,
        }
    }

    pub fn refresh(&self) {
        debug!("LLM refresh requested");
        let _ = self.refresh_tx.try_send(());
    }

    pub fn try_get_result(&self) -> Option<LLMResult> {
        // Check for errors first
        if let Some(error) = self.llm_state.get_error("advice_generation") {
            // Clear the error after reading it
            self.llm_state.clear_error("advice_generation");
            return Some(LLMResult::Error(error));
        }

        // Check for current advice
        if let Some(advice) = self.llm_state.get_current_advice("current") {
            return Some(LLMResult::Success(advice));
        }

        // No result available
        None
    }
}
