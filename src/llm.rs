use crate::config::LlmConfig;

use crate::shared_state::LlmSharedState;
use log::debug;
use openai_api_rs::v1::api::OpenAIClient;
use openai_api_rs::v1::chat_completion::{self, ChatCompletionRequest};
use std::env;

use std::sync::Arc;
use tokio::sync::Mutex;



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
    llm_state: Arc<LlmSharedState>,
}



impl AsyncLLMCommand {
    pub fn new(_llm_client: LlmClient, llm_state: Arc<LlmSharedState>) -> Self {
        let _llm_state_clone = Arc::clone(&llm_state);

        tokio::spawn(async move {
            loop {
                // Check if there's a git repo available in shared state
                // This is a simplified approach - in practice, you might want to 
                // implement a more sophisticated triggering mechanism
                tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;
                
                // For now, this is a placeholder implementation
                // The actual advice generation should be triggered by external events
                debug!("LLM worker running in background");
            }
        });

        Self {
            llm_state,
        }
    }

    pub fn refresh(&self) {
        debug!("LLM refresh requested - using shared state");
        // Refresh logic would be implemented here using shared state
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
}
