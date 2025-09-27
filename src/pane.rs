use std::collections::HashMap;

use log::debug;
use md5;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
    prelude::Stylize,
};

use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::shared_state::LlmSharedState;
use crate::ui::{ActivePane, App, Theme};
use std::sync::Arc;

pub trait Pane {
    fn title(&self) -> String;
    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>>;
    fn handle_event(&mut self, event: &AppEvent) -> bool;
    fn visible(&self) -> bool;
    fn set_visible(&mut self, visible: bool);
    fn as_commit_picker_pane(&self) -> Option<&CommitPickerPane> {
        None
    }
    fn as_commit_picker_pane_mut(&mut self) -> Option<&mut CommitPickerPane> {
        None
    }
    #[allow(dead_code)]
    fn as_commit_summary_pane(&self) -> Option<&CommitSummaryPane> {
        None
    }
    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        None
    }
    fn as_advice_pane(&self) -> Option<&AdvicePanel> {
        None
    }
    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePanel> {
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    FileTree,
    Monitor,
    Diff,
    SideBySideDiff,
    Help,
    StatusBar,
    CommitPicker,
    CommitSummary,
    Advice,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum AppEvent {
    Key(KeyEvent),
    DataUpdated((), String),
    ThemeChanged(()),
}

// Advice Panel Data Structures
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdviceMode {
    Chatting,
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImprovementPriority {
    Low,
    Medium,
    High,
    Critical,
    Unknown,
}

impl std::fmt::Display for ImprovementPriority {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImprovementPriority::Low => write!(f, "Low"),
            ImprovementPriority::Medium => write!(f, "Medium"),
            ImprovementPriority::High => write!(f, "High"),
            ImprovementPriority::Critical => write!(f, "Critical"),
            ImprovementPriority::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AdviceImprovement {
    pub id: String,
    pub title: String,
    pub description: String,
    pub priority: ImprovementPriority,
    pub category: String,
    pub code_examples: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ChatMessageData {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub timestamp: std::time::SystemTime,
}

#[derive(Debug, Clone)]
pub enum AdviceContent {
    Improvements(Vec<AdviceImprovement>),
    Chat(Vec<ChatMessageData>),
    Help(String),
    Loading,
    Error(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadingState {
    Idle,
    GeneratingAdvice,
    SendingChat,
    LoadingHelp,
}

#[derive(Debug)]
pub struct AdvicePanel {
    pub visible: bool,
    pub mode: AdviceMode,
    pub content: AdviceContent,
    pub chat_input: String,
    pub chat_input_active: bool,
    pub config: crate::config::AdviceConfig,
    pub scroll_offset: usize,
    pub shared_state: Option<std::sync::Arc<crate::shared_state::LlmSharedState>>,
    pub llm_client: Option<std::sync::Arc<tokio::sync::Mutex<LlmClient>>>,
    pub current_diff_hash: Option<String>,
    pub loading_state: LoadingState,
    pub pending_advice_task: Option<tokio::task::JoinHandle<()>>,
    pub pending_chat_task: Option<tokio::task::JoinHandle<()>>,
    pub pending_chat_message_id: Option<String>,
    pub current_diff_content: std::cell::RefCell<Option<String>>,
    pub initial_message_sent: bool,
    pub first_visit: bool,
    pub chat_content_backup: Option<AdviceContent>,
}

impl AdvicePanel {
    pub fn new(_config: crate::config::Config, advice_config: crate::config::AdviceConfig) -> Result<Self, String> {
        Ok(Self {
            visible: false,
            mode: AdviceMode::Chatting,
            content: AdviceContent::Loading,
            chat_input: String::new(),
            chat_input_active: false,
            config: advice_config,
            scroll_offset: 0,
            shared_state: None,
            llm_client: None,
            current_diff_hash: None,
            loading_state: LoadingState::Idle,
            pending_advice_task: None,
            pending_chat_task: None,
            pending_chat_message_id: None,
            current_diff_content: std::cell::RefCell::new(None),
            initial_message_sent: false,
            first_visit: true,
            chat_content_backup: None,
        })
    }

    /// Set the shared state for the advice panel
    pub fn set_shared_state(&mut self, shared_state: std::sync::Arc<crate::shared_state::LlmSharedState>) {
        self.shared_state = Some(shared_state);
    }

    /// Set the LLM client for the advice panel
    pub fn set_llm_client(&mut self, llm_client: std::sync::Arc<tokio::sync::Mutex<LlmClient>>) {
        debug!("🎯 ADVICE_PANEL: LLM client has been set");
        self.llm_client = Some(llm_client);
    }

    pub fn get_mode(&self) -> AdviceMode {
        self.mode
    }

    pub fn get_chat_history(&self) -> Vec<ChatMessageData> {
        match &self.content {
            AdviceContent::Chat(messages) => messages.clone(),
            _ => Vec::new(),
        }
    }

    pub fn get_improvements(&self) -> Vec<AdviceImprovement> {
        match &self.content {
            AdviceContent::Improvements(improvements) => improvements.clone(),
            _ => Vec::new(),
        }
    }

    pub fn generate_advice(&mut self, diff: &str) -> Result<Vec<AdviceImprovement>, String> {
        if diff.is_empty() {
            return Ok(Vec::new());
        }

        // Create diff hash for caching
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        diff.hash(&mut hasher);
        let diff_hash = format!("{:x}", hasher.finish());

        self.current_diff_hash = Some(diff_hash.clone());

        // Check if we have cached advice
        if let Some(cached_json) = self.load_cached_advice(&diff_hash) {
            debug!("🎯 ADVICE_PANEL: Using cached advice for diff hash: {}", diff_hash);
            return self.parse_cached_advice(&cached_json);
        }

        // Check if already generating
        if self.is_generating_advice() {
            return Err("Advice generation already in progress".to_string());
        }

        // Start async generation
        self.start_async_advice_generation(diff)?;
        Ok(Vec::new()) // Return empty for now, will be populated async
    }

    pub fn send_chat_message(&mut self, message: &str) -> Result<(), String> {
        debug!("🎯 ADVICE_PANEL: Sending chat message: {}", message);

        // Add user message to chat history immediately (preserves content)
        let user_message_id = uuid::Uuid::new_v4().to_string();
        let user_message = ChatMessageData {
            id: user_message_id.clone(),
            role: MessageRole::User,
            content: message.to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        // Update chat history (never clear, only append)
        match &mut self.content {
            AdviceContent::Chat(messages) => {
                messages.push(user_message);
            }
            _ => {
                // If not in chat mode, initialize chat content
                self.content = AdviceContent::Chat(vec![user_message]);
            }
        }

        // Set loading state to show "thinking" indicator
        self.update_advice_status(LoadingState::SendingChat);

        // Get context for the LLM
        let original_diff = self.get_current_diff_context().unwrap_or_default();
        let conversation_history = self.get_chat_history();

        // Store the message ID for tracking the response
        self.pending_chat_message_id = Some(user_message_id.clone());

        // Clone necessary data for the async task
        let shared_state_clone = self.shared_state.clone();
        let llm_client_clone = self.llm_client.clone();
        let message_id_clone = user_message_id.clone();
        let message_content = message.to_string();
        let diff_content = original_diff.clone();

        // Spawn async task for chat response generation
        let task = tokio::spawn(async move {
            debug!("🎯 ADVICE_PANEL: Async chat task started for message: {}", message_content);

            let result = async {
                // Try to use LLM client if available
                if let Some(llm_client) = llm_client_clone {
                    let client = llm_client.lock().await;
                    debug!("🎯 ADVICE_PANEL: About to call LLM send_chat_followup");

                    match client.send_chat_followup(message_content, conversation_history, diff_content).await {
                        Ok(ai_message) => {
                            debug!("🎯 ADVICE_PANEL: Successfully generated AI chat response");
                            Ok(ai_message)
                        }
                        Err(e) => {
                            debug!("🎯 ADVICE_PANEL: LLM send_chat_followup failed: {}", e);
                            Err(format!("LLM chat request failed: {}", e))
                        }
                    }
                } else {
                    Err("LLM client not available".to_string())
                }
            }.await;

            // Store results in shared state
            if let Some(shared_state) = shared_state_clone {
                match result {
                    Ok(ai_message) => {
                        shared_state.store_pending_chat_response(message_id_clone.clone(), ai_message);
                        debug!("🎯 ADVICE_PANEL: Stored chat response in shared state");
                    }
                    Err(e) => {
                        shared_state.set_advice_error(format!("chat_{}", message_id_clone), e);
                        debug!("🎯 ADVICE_PANEL: Stored chat error in shared state");
                    }
                }
            }
        });

        self.pending_chat_task = Some(task);
        debug!("🎯 ADVICE_PANEL: Spawned async chat generation task with message ID: {}", user_message_id);

        Ok(())
    }

    
    pub fn clear_chat_history(&mut self) -> Result<(), String> {
        if let AdviceContent::Chat(messages) = &mut self.content {
            messages.clear();
        }
        Ok(())
    }

    pub fn start_async_advice_generation(&mut self, diff: &str) -> Result<(), String> {
        // Update loading state
        self.update_advice_status(LoadingState::GeneratingAdvice);
        self.content = AdviceContent::Loading;

        // Use the new async architecture via trigger_initial_advice
        // Set the current diff hash manually since we're not going through the normal trigger flow
        let diff_hash = format!("{:x}", md5::compute(diff.as_bytes()));
        self.current_diff_hash = Some(diff_hash.clone());

        // Check if we already have results for this diff
        if let Some(shared_state) = &self.shared_state {
            if let Some(cached_results) = shared_state.get_advice_results(&diff_hash) {
                debug!("🎯 ADVICE_PANEL: Found cached advice results for diff hash: {}", diff_hash);
                self.content = AdviceContent::Improvements(cached_results);
                self.update_advice_status(LoadingState::Idle);
                return Ok(());
            }
        }

        // Spawn async task for LLM advice generation
        let shared_state_clone = self.shared_state.clone();
        let llm_client_clone = self.llm_client.clone();
        let diff_hash_clone = diff_hash.clone();
        let diff_content = diff.to_string();

        let task = tokio::spawn(async move {
            debug!("🎯 ADVICE_PANEL: Async advice task started with {} chars", diff_content.len());

            let result = async {
                // Try to use LLM client if available
                if let Some(llm_client) = llm_client_clone {
                    let client = llm_client.lock().await;
                    debug!("🎯 ADVICE_PANEL: About to call LLM generate_advice");

                    match client.generate_advice(diff_content, 3).await {
                        Ok(improvements) => {
                            debug!("🎯 ADVICE_PANEL: Successfully generated {} improvements", improvements.len());
                            Ok(improvements)
                        }
                        Err(e) => {
                            debug!("🎯 ADVICE_PANEL: LLM generate_advice failed: {}", e);
                            Err(format!("LLM request failed: {}", e))
                        }
                    }
                } else {
                    Err("LLM client not available".to_string())
                }
            }.await;

            // Store results in shared state
            if let Some(shared_state) = shared_state_clone {
                match result {
                    Ok(improvements) => {
                        shared_state.store_advice_results(diff_hash_clone.clone(), improvements);
                        debug!("🎯 ADVICE_PANEL: Stored advice results in shared state");
                    }
                    Err(e) => {
                        shared_state.set_advice_error(format!("advice_{}", diff_hash_clone), e);
                        debug!("🎯 ADVICE_PANEL: Stored error in shared state");
                    }
                }
            }
        });

        self.pending_advice_task = Some(task);
        debug!("🎯 ADVICE_PANEL: Started async advice generation via start_async_advice_generation");

        Ok(())
    }

    /// Get the current diff context for LLM
    fn get_current_diff_context(&self) -> Option<String> {
        // Return the stored diff content that was updated during rendering
        self.current_diff_content.borrow().clone()
    }

    
    /// Check and update pending async tasks
    pub fn check_pending_tasks(&mut self) {
        // Check if async advice task has completed
        if let Some(task) = self.pending_advice_task.take() {
            if task.is_finished() {
                debug!("🎯 ADVICE_PANEL: Pending advice task completed");
                // Task is finished, check shared state for results
                self.update_content_from_shared_state();
            } else {
                // Task is still running, put it back
                self.pending_advice_task = Some(task);
            }
        }

        // Check if async chat task has completed
        if let Some(task) = self.pending_chat_task.take() {
            if task.is_finished() {
                debug!("🎯 ADVICE_PANEL: Pending chat task completed");
                // Task is finished, check shared state for chat response
                self.update_chat_from_shared_state();
            } else {
                // Task is still running, put it back
                self.pending_chat_task = Some(task);
            }
        }
    }

    /// Update panel content from shared state when async tasks complete
    fn update_content_from_shared_state(&mut self) {
        if let (Some(shared_state), Some(diff_hash)) = (&self.shared_state, &self.current_diff_hash) {
            // Check if we have results for this diff
            if let Some(results) = shared_state.get_advice_results(diff_hash) {
                debug!("🎯 ADVICE_PANEL: Updating panel with {} improvements from shared state", results.len());
                self.content = AdviceContent::Improvements(results);
                self.update_advice_status(LoadingState::Idle);
            } else if let Some(error) = shared_state.get_advice_error(&format!("advice_{}", diff_hash)) {
                debug!("🎯 ADVICE_PANEL: Updating panel with error from shared state: {}", error);
                let improvements = vec![
                    AdviceImprovement {
                        id: uuid::Uuid::new_v4().to_string(),
                        title: "Advice Generation Failed".to_string(),
                        description: format!("Failed to generate advice: {}", error),
                        priority: ImprovementPriority::High,
                        category: "Error".to_string(),
                        code_examples: Vec::new(),
                    },
                ];
                self.content = AdviceContent::Improvements(improvements);
                self.update_advice_status(LoadingState::Idle);
            }
        }
    }

    /// Update chat content from shared state when async chat tasks complete
    fn update_chat_from_shared_state(&mut self) {
        // Take ownership of the pending message ID to avoid borrow conflicts
        if let Some(message_id) = self.pending_chat_message_id.take() {
            // Clone shared_state to avoid borrow conflicts
            let shared_state_clone = self.shared_state.clone();

            if let Some(shared_state) = &shared_state_clone {
                // Check if we have a chat response for this message
                if let Some(response) = shared_state.get_pending_chat_response(&message_id) {
                    debug!("🎯 ADVICE_PANEL: Updating chat with AI response from shared state");

                    // Add the AI response to the chat
                    if let AdviceContent::Chat(messages) = &mut self.content {
                        messages.push(response);
                    }

                    // Reset loading state
                    self.update_advice_status(LoadingState::Idle);

                    // Clean up the shared state
                    shared_state.remove_pending_chat_response(&message_id);
                } else if let Some(error) = shared_state.get_advice_error(&format!("chat_{}", message_id)) {
                    debug!("🎯 ADVICE_PANEL: Updating chat with error from shared state: {}", error);

                    // Add error message to chat
                    let error_message = ChatMessageData {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: MessageRole::Assistant,
                        content: format!("Sorry, I encountered an error: {}", error),
                        timestamp: std::time::SystemTime::now(),
                    };

                    if let AdviceContent::Chat(messages) = &mut self.content {
                        messages.push(error_message);
                    }

                    // Reset loading state
                    self.update_advice_status(LoadingState::Idle);

                    // Clean up the shared state
                    shared_state.clear_advice_error(&format!("chat_{}", message_id));
                } else {
                    // If no response or error found, put the message ID back
                    self.pending_chat_message_id = Some(message_id);
                }
            } else {
                // If no shared state, put the message ID back
                self.pending_chat_message_id = Some(message_id);
            }
        }
    }

    /// Trigger initial advice generation when panel opens (spawns async task)
    fn send_initial_message_with_diff(&mut self, diff_content: &str) {
        debug!("🎯 ADVICE_PANEL: Sending initial message with diff context");

        // Always start in chat mode with empty history
        self.mode = AdviceMode::Chatting;
        self.content = AdviceContent::Chat(Vec::new());

        let initial_message = format!(
            "Please provide 3 actionable improvements for the following code changes:\n\n```diff\n{}\n```\n\nFocus on practical, specific suggestions that would improve code quality, performance, or maintainability.",
            diff_content
        );

        // Send the initial message automatically
        if let Err(e) = self.send_chat_message(&initial_message) {
            // If sending fails, add an error message
            let error_message = ChatMessageData {
                id: uuid::Uuid::new_v4().to_string(),
                role: MessageRole::System,
                content: format!("Failed to send initial request to AI: {}", e),
                timestamp: std::time::SystemTime::now(),
            };

            if let AdviceContent::Chat(messages) = &mut self.content {
                messages.push(error_message);
            }
            self.update_advice_status(LoadingState::Idle);
        } else {
            debug!("🎯 ADVICE_PANEL: Successfully sent initial chat message for advice");
        }
    }

    fn send_no_changes_message(&mut self) {
        debug!("🎯 ADVICE_PANEL: Sending no changes message");

        // Always start in chat mode with empty history
        self.mode = AdviceMode::Chatting;
        self.content = AdviceContent::Chat(Vec::new());

        // Add a system message about no changes
        let system_message = ChatMessageData {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content: "No code changes are currently available to analyze. Make some code changes and stage them with `git add` to get AI-powered improvement suggestions. You can still ask me general questions about programming best practices!".to_string(),
            timestamp: std::time::SystemTime::now(),
        };

        if let AdviceContent::Chat(messages) = &mut self.content {
            messages.push(system_message);
        }
    }

    fn refresh_chat_with_new_diff(&mut self) {
        debug!("🎯 ADVICE_PANEL: Refreshing chat with new diff");

        // Clear existing chat content
        self.content = AdviceContent::Chat(Vec::new());
        self.scroll_offset = 0;

        // Reset first visit flag so it will send a new initial message
        self.first_visit = true;
        self.initial_message_sent = false;

        // The new message will be sent on the next render when we have fresh diff data
    }

    fn trigger_initial_advice(&mut self) {
        debug!("🎯 ADVICE_PANEL: Triggering initial chat message");

        // Always start in chat mode with empty history
        self.mode = AdviceMode::Chatting;
        self.content = AdviceContent::Chat(Vec::new());

        // Get current diff for context
        let diff_content = self.get_current_diff_context().unwrap_or_default();

        if diff_content.is_empty() {
            // No diff available, add a system message
            let system_message = ChatMessageData {
                id: uuid::Uuid::new_v4().to_string(),
                role: MessageRole::System,
                content: "No code changes are currently available to analyze. Make some code changes and stage them with `git add` to get AI-powered improvement suggestions. You can still ask me general questions about programming best practices!".to_string(),
                timestamp: std::time::SystemTime::now(),
            };

            if let AdviceContent::Chat(messages) = &mut self.content {
                messages.push(system_message);
            }
            self.update_advice_status(LoadingState::Idle);
            return;
        }

        // Send initial message asking for 3 actionable improvements
        let initial_message = format!(
            "Please provide 3 actionable improvements for the following code changes:\n\n```diff\n{}\n```\n\nFocus on practical, specific suggestions that would improve code quality, performance, or maintainability.",
            diff_content
        );

        // Send the initial message automatically
        if let Err(e) = self.send_chat_message(&initial_message) {
            // If sending fails, add an error message
            let error_message = ChatMessageData {
                id: uuid::Uuid::new_v4().to_string(),
                role: MessageRole::System,
                content: format!("Failed to send initial request to AI: {}", e),
                timestamp: std::time::SystemTime::now(),
            };

            if let AdviceContent::Chat(messages) = &mut self.content {
                messages.push(error_message);
            }
            self.update_advice_status(LoadingState::Idle);
        } else {
            debug!("🎯 ADVICE_PANEL: Successfully sent initial chat message for advice");
        }
    }

    fn trigger_initial_advice_old(&mut self) {
        debug!("🎯 ADVICE_PANEL: Triggering initial advice generation");

        // Set loading state
        self.update_advice_status(LoadingState::GeneratingAdvice);
        self.content = AdviceContent::Loading;

        // Get current diff for advice generation
        let diff_content = self.get_current_diff_context().unwrap_or_default();

        if diff_content.is_empty() {
            // No diff available, show helpful message
            let improvements = vec![
                AdviceImprovement {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: "No Code Changes Available".to_string(),
                    description: "No git diff is currently available to analyze. Make some code changes and stage them to see AI-powered improvement suggestions.".to_string(),
                    priority: ImprovementPriority::Medium,
                    category: "Info".to_string(),
                    code_examples: Vec::new(),
                },
                AdviceImprovement {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: "How to Use This Panel".to_string(),
                    description: "1. Make code changes in your repository\n2. Stage your changes with `git add`\n3. Press Ctrl+L to open this advice panel\n4. Review AI-generated improvement suggestions\n5. Press '/' to chat with AI about specific changes".to_string(),
                    priority: ImprovementPriority::Low,
                    category: "Guide".to_string(),
                    code_examples: Vec::new(),
                },
            ];
            self.content = AdviceContent::Improvements(improvements);
            self.update_advice_status(LoadingState::Idle);
            return;
        }

        // Generate diff hash for this request
        let diff_hash = format!("{:x}", md5::compute(diff_content.as_bytes()));
        let diff_len = diff_content.len();
        self.current_diff_hash = Some(diff_hash.clone());
        debug!("🎯 ADVICE_PANEL: Generated diff hash: {}", diff_hash);

        // Check if we already have results for this diff
        if let Some(shared_state) = &self.shared_state {
            if let Some(cached_results) = shared_state.get_advice_results(&diff_hash) {
                debug!("🎯 ADVICE_PANEL: Found cached advice results for diff hash: {}", diff_hash);
                self.content = AdviceContent::Improvements(cached_results);
                self.update_advice_status(LoadingState::Idle);
                return;
            }
        }

        // Track this advice generation task in shared state
        if let Some(shared_state) = &self.shared_state {
            shared_state.start_advice_task(diff_hash.clone());
        }

        // Clone necessary data for the async task
        let shared_state_clone = self.shared_state.clone();
        let llm_client_clone = self.llm_client.clone();
        let diff_hash_clone = diff_hash.clone();

        // Spawn async task for LLM advice generation
        let task = tokio::spawn(async move {
            debug!("🎯 ADVICE_PANEL: Async advice task started with {} chars", diff_content.len());

            let result = async {
                // Try to use LLM client if available
                if let Some(llm_client) = llm_client_clone {
                    let client = llm_client.lock().await;
                    debug!("🎯 ADVICE_PANEL: About to call LLM generate_advice");

                    match client.generate_advice(diff_content, 3).await {
                        Ok(improvements) => {
                            debug!("🎯 ADVICE_PANEL: Successfully generated {} improvements", improvements.len());
                            Ok(improvements)
                        }
                        Err(e) => {
                            debug!("🎯 ADVICE_PANEL: LLM generate_advice failed: {}", e);
                            Err(format!("LLM request failed: {}", e))
                        }
                    }
                } else {
                    Err("LLM client not available".to_string())
                }
            }.await;

            // Store results in shared state
            if let Some(shared_state) = shared_state_clone {
                match result {
                    Ok(improvements) => {
                        shared_state.store_advice_results(diff_hash_clone.clone(), improvements);
                        debug!("🎯 ADVICE_PANEL: Stored advice results in shared state");
                    }
                    Err(e) => {
                        shared_state.set_advice_error(format!("advice_{}", diff_hash_clone), e);
                        debug!("🎯 ADVICE_PANEL: Stored error in shared state");
                    }
                }

                // Mark task as completed
                shared_state.complete_advice_task(&diff_hash_clone);
            }
        });

        self.pending_advice_task = Some(task);
        debug!("🎯 ADVICE_PANEL: Spawned async advice generation task with shared state communication");

        // For now, use the fallback improvements while the async task runs
        let improvements = vec![
            AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Async Advice Generation Started".to_string(),
                description: "An asynchronous task has been spawned to generate AI-powered advice. The results will appear here momentarily. This demonstrates the new async architecture working properly.".to_string(),
                priority: ImprovementPriority::Medium,
                category: "System".to_string(),
                code_examples: Vec::new(),
            },
            AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Diff Analysis in Progress".to_string(),
                description: format!("Currently analyzing diff with {} characters. The async architecture allows the UI to remain responsive while LLM processing happens in the background.", diff_len),
                priority: ImprovementPriority::Low,
                category: "Info".to_string(),
                code_examples: Vec::new(),
            },
        ];
        self.content = AdviceContent::Improvements(improvements);
        self.update_advice_status(LoadingState::Idle);
    }

    
    pub fn get_advice_generation_status(&self) -> String {
        match self.loading_state {
            LoadingState::GeneratingAdvice => "Generating advice...".to_string(),
            LoadingState::SendingChat => "Sending message...".to_string(),
            LoadingState::LoadingHelp => "Loading help...".to_string(),
            LoadingState::Idle => "Ready".to_string(),
        }
    }

    pub fn get_last_chat_error(&self) -> Option<String> {
        // Check shared state for errors
        if let Some(ref shared_state) = self.shared_state {
            shared_state.get_advice_error("chat_error")
        } else {
            None
        }
    }

    /// Update the advice generation status
    pub fn update_advice_status(&mut self, status: LoadingState) {
        let old_status = self.loading_state.clone();
        self.loading_state = status.clone();

        // Update shared state if available
        if let Some(ref shared_state) = self.shared_state {
            match status {
                LoadingState::GeneratingAdvice => {
                    if let Some(ref diff_hash) = self.current_diff_hash {
                        shared_state.start_advice_task(diff_hash.clone());
                    }
                }
                LoadingState::Idle => {
                    if old_status == LoadingState::GeneratingAdvice {
                        if let Some(ref diff_hash) = self.current_diff_hash {
                            shared_state.complete_advice_task(diff_hash);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Get the current loading state
    pub fn get_loading_state(&self) -> LoadingState {
        self.loading_state.clone()
    }

    /// Check if advice is currently being generated
    pub fn is_generating_advice(&self) -> bool {
        match self.loading_state {
            LoadingState::GeneratingAdvice => true,
            _ => false,
        }
    }

    /// Cache advice in shared state
    pub fn cache_advice(&self, diff_hash: &str, advice: &str) {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.cache_advice(diff_hash.to_string(), advice.to_string());
        }
    }

    /// Load cached advice from shared state
    pub fn load_cached_advice(&self, diff_hash: &str) -> Option<String> {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.get_cached_advice(diff_hash)
        } else {
            None
        }
    }

    /// Save chat session to shared state
    pub fn save_chat_session(&self, session_id: &str, chat_history: &str) {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.save_chat_session(session_id.to_string(), chat_history.to_string());
        }
    }

    /// Load chat session from shared state
    pub fn load_chat_session(&self, session_id: &str) -> Option<String> {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.load_chat_session(session_id)
        } else {
            None
        }
    }

    /// Set advice panel error
    pub fn set_advice_error(&self, key: &str, error: &str) {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.set_advice_error(key.to_string(), error.to_string());
        }
    }

    /// Clear advice panel error
    pub fn clear_advice_error(&self, key: &str) {
        if let Some(ref shared_state) = self.shared_state {
            shared_state.clear_advice_error(key);
        }
    }

    /// Generate advice asynchronously
    async fn generate_advice_async(&mut self, diff: &str) -> Result<(), String> {
        debug!("🎯 ADVICE_PANEL: Generating advice asynchronously");

        // Set loading state
        self.content = AdviceContent::Loading;
        self.update_advice_status(LoadingState::GeneratingAdvice);

        // Try to use LLM client
        let llm_client_clone = self.llm_client.clone();
        debug!("🎯 ADVICE_PANEL: LLM client available: {}", llm_client_clone.is_some());
        if let Some(llm_client) = llm_client_clone {
            let client = llm_client.lock().await;
            debug!("🎯 ADVICE_PANEL: About to call LLM generate_advice with diff length: {}", diff.len());

            // Generate advice using LLM client
            match client.generate_advice(diff.to_string(), 3).await {
                Ok(improvements) => {
                    debug!("🎯 ADVICE_PANEL: Successfully generated {} improvements via LLM", improvements.len());

                    // Cache the improvements
                    if let Some(ref diff_hash) = self.current_diff_hash {
                        if let Ok(cached_json) = serde_json::to_string(&improvements) {
                            self.cache_advice(diff_hash, &cached_json);
                        }
                    }

                    // Update content
                    self.content = AdviceContent::Improvements(improvements);
                    self.update_advice_status(LoadingState::Idle);
                    return Ok(());
                }
                Err(error) => {
                    debug!("🎯 ADVICE_PANEL: LLM generation failed, using fallback: {}", error);
                    // Fall back to placeholder improvements
                }
            }
        }

        debug!("🎯 ADVICE_PANEL: Using fallback improvements (LLM client not available)");

        // Fallback improvements when LLM is not available
        let improvements = vec![
            AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Code Quality Improvement".to_string(),
                description: "The diff shows opportunities for improving code quality and maintainability.".to_string(),
                priority: ImprovementPriority::Medium,
                category: "CodeQuality".to_string(),
                code_examples: Vec::new(),
            },
            AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Performance Optimization".to_string(),
                description: "Consider optimizing the algorithm or data structures for better performance.".to_string(),
                priority: ImprovementPriority::Medium,
                category: "Performance".to_string(),
                code_examples: Vec::new(),
            },
            AdviceImprovement {
                id: uuid::Uuid::new_v4().to_string(),
                title: "Error Handling".to_string(),
                description: "Add proper error handling to make the code more robust.".to_string(),
                priority: ImprovementPriority::Low,
                category: "BugFix".to_string(),
                code_examples: Vec::new(),
            },
        ];

        // Cache the improvements
        if let Some(ref diff_hash) = self.current_diff_hash {
            if let Ok(cached_json) = serde_json::to_string(&improvements) {
                self.cache_advice(diff_hash, &cached_json);
            }
        }

        // Update content
        self.content = AdviceContent::Improvements(improvements);
        self.update_advice_status(LoadingState::Idle);

        Ok(())
    }

    /// Parse cached advice from JSON
    fn parse_cached_advice(&self, cached_json: &str) -> Result<Vec<AdviceImprovement>, String> {
        match serde_json::from_str::<Vec<AdviceImprovement>>(cached_json) {
            Ok(improvements) => Ok(improvements),
            Err(e) => Err(format!("Failed to parse cached advice: {}", e)),
        }
    }

    pub fn is_chat_available(&self) -> bool {
        true
    }

    pub fn get_visibility(&self) -> bool {
        self.visible
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    pub fn set_visibility(&mut self, visible: bool) {
        let was_visible = self.visible;
        self.visible = visible;

        // When panel becomes visible, set up chat input state but preserve history
        if visible && !was_visible {
            debug!("🎯 ADVICE_PANEL: Panel became visible");
            self.chat_input_active = false;
        }
    }
}

pub struct PaneRegistry {
    panes: HashMap<PaneId, Box<dyn Pane>>,
    theme: Theme,
}

impl std::fmt::Debug for PaneRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneRegistry")
            .field("pane_count", &self.panes.len())
            .field("theme", &self.theme)
            .finish()
    }
}

impl PaneRegistry {
    pub fn new(theme: Theme, llm_client: LlmClient, llm_shared_state: Arc<LlmSharedState>) -> Self {
        let mut registry = Self {
            panes: HashMap::new(),
            theme,
        };

        registry.register_default_panes(llm_client, llm_shared_state);
        registry
    }

    fn register_default_panes(
        &mut self,
        llm_client: LlmClient,
        llm_shared_state: Arc<LlmSharedState>,
    ) {
        self.register_pane(PaneId::FileTree, Box::new(FileTreePane::new()));
        self.register_pane(PaneId::Monitor, Box::new(MonitorPane::new()));
        self.register_pane(PaneId::Diff, Box::new(DiffPane::new()));
        self.register_pane(PaneId::SideBySideDiff, Box::new(SideBySideDiffPane::new()));
        self.register_pane(PaneId::Help, Box::new(HelpPane::new()));
        self.register_pane(PaneId::StatusBar, Box::new(StatusBarPane::new()));
        self.register_pane(PaneId::CommitPicker, Box::new(CommitPickerPane::new()));
        let mut commit_summary_pane = CommitSummaryPane::new_with_llm_client(Some(llm_client.clone()));
        commit_summary_pane.set_shared_state(llm_shared_state.clone());
        self.register_pane(PaneId::CommitSummary, Box::new(commit_summary_pane));

        // Create advice panel with configuration, LLM client, and shared state
        let advice_config = crate::config::AdviceConfig::default();
        let mut advice_panel = AdvicePanel::new(crate::config::Config::default(), advice_config)
            .expect("Failed to create AdvicePanel");
        advice_panel.set_shared_state(llm_shared_state.clone());
        advice_panel.set_llm_client(std::sync::Arc::new(tokio::sync::Mutex::new(llm_client.clone())));
        self.register_pane(PaneId::Advice, Box::new(advice_panel));
    }

    pub fn register_pane(&mut self, id: PaneId, pane: Box<dyn Pane>) {
        self.panes.insert(id, pane);
    }

    pub fn get_pane(&self, id: &PaneId) -> Option<&dyn Pane> {
        self.panes.get(id).map(|p| p.as_ref())
    }

    pub fn with_pane_mut<F, R>(&mut self, id: &PaneId, f: F) -> Option<R>
    where
        F: FnOnce(&mut dyn Pane) -> R,
    {
        self.panes.get_mut(id).map(|p| f(p.as_mut()))
    }

    pub fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        pane_id: PaneId,
        git_repo: &GitRepo,
    ) {
        if let Some(pane) = self.get_pane(&pane_id)
            && pane.visible()
            && let Err(e) = pane.render(f, app, area, git_repo)
        {
            log::error!("Error rendering pane {pane_id:?}: {e}");
        }
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        // Notify all panes of theme change
        let event = AppEvent::ThemeChanged(());
        for pane in self.panes.values_mut() {
            let _ = pane.handle_event(&event);
        }
    }
}

// File Tree Pane Implementation
pub struct FileTreePane {
    visible: bool,
    scroll_offset: usize,
}

impl Default for FileTreePane {
    fn default() -> Self {
        Self::new()
    }
}

impl FileTreePane {
    pub fn new() -> Self {
        Self {
            visible: true,
            scroll_offset: 0,
        }
    }
}

impl Pane for FileTreePane {
    fn title(&self) -> String {
        "Changed Files".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{List, ListItem},
        };

        let theme = app.get_theme();
        let tree_items: Vec<ListItem> = app
            .get_tree_nodes()
            .iter()
            .enumerate()
            .map(|(index, (node, depth))| {
                let indent = "  ".repeat(*depth);
                let name_spans = if node.is_dir {
                    vec![Span::raw(format!("{}📁 {}", indent, node.name))]
                } else {
                    let mut spans = Vec::new();

                    // Add arrow for current file selection
                    if index == app.get_current_tree_index() {
                        spans.push(Span::styled(
                            "-> ",
                            Style::default()
                                .fg(theme.secondary_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    } else {
                        spans.push(Span::raw("   "));
                    }

                    let status_char = if let Some(ref diff) = node.file_diff {
                        if diff.status.is_wt_new() {
                            "📄 "
                        } else if diff.status.is_wt_modified() {
                            "📝 "
                        } else if diff.status.is_wt_deleted() {
                            "🗑️  "
                        } else {
                            "📄 "
                        }
                    } else {
                        "📄 "
                    };

                    spans.push(Span::raw(format!("{indent}{status_char}")));
                    spans.push(Span::raw(node.name.clone()));

                    if let Some(ref diff) = node.file_diff {
                        if diff.additions > 0 {
                            spans.push(Span::styled(
                                format!(" (+{})", diff.additions),
                                Style::default()
                                    .fg(theme.added_color())
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                        if diff.deletions > 0 {
                            spans.push(Span::styled(
                                format!(" (-{})", diff.deletions),
                                Style::default()
                                    .fg(theme.removed_color())
                                    .add_modifier(Modifier::BOLD),
                            ));
                        }
                    }

                    spans
                };

                let line_style = if let Some(ref diff) = node.file_diff {
                    // Check if this file is recently changed by finding its index
                    if let Some(file_idx) = app.get_files().iter().position(|f| f.path == diff.path)
                    {
                        if file_idx < app.get_file_change_timestamps().len()
                            && app.is_file_recently_changed(file_idx)
                        {
                            // Recently changed - highlight
                            Style::default()
                                .fg(theme.foreground_color())
                                .bg(theme.highlight_color())
                                .add_modifier(Modifier::BOLD)
                        } else {
                            // Not recently changed - normal
                            Style::default().fg(theme.foreground_color())
                        }
                    } else {
                        // File not found in files list - normal
                        Style::default().fg(theme.foreground_color())
                    }
                } else {
                    // Directory
                    Style::default()
                        .fg(theme.directory_color())
                        .add_modifier(Modifier::BOLD)
                };

                let line = Line::from(name_spans).style(line_style);
                ListItem::new(line)
            })
            .collect();

        let file_list = List::new(tree_items)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .highlight_style(
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(file_list, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => {
                // Handle key events for file tree navigation
                match key.code {
                    KeyCode::Char('j') => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                        true
                    }
                    KeyCode::Char('k') => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Monitor Pane Implementation
pub struct MonitorPane {
    visible: bool,
    scroll_offset: usize,
    output: String,
}

impl Default for MonitorPane {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            scroll_offset: 0,
            output: String::new(),
        }
    }

    pub fn update_output(&mut self, output: String) {
        self.output = output;
    }
}

impl Pane for MonitorPane {
    fn title(&self) -> String {
        "Monitor".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };
        let theme = app.get_theme();
        let monitor_lines: Vec<_> = self.output.lines().skip(self.scroll_offset).collect();
        let visible_lines = area.height.saturating_sub(2) as usize;

        let display_lines: Vec<Line> = monitor_lines
            .iter()
            .take(visible_lines)
            .map(|line| {
                Line::from(Span::styled(
                    line.to_string(),
                    Style::default().fg(theme.foreground_color()),
                ))
            })
            .collect();

        let title = if !app.get_monitor_command_configured() {
            "Monitor (no command configured)".to_string()
        } else if !app.get_monitor_has_run() {
            "Monitor ⏳ loading...".to_string()
        } else if let Some(elapsed) = app.get_monitor_elapsed_time() {
            let time_str = app.format_elapsed_time(elapsed);
            format!("Monitor ⏱️ {time_str} ago")
        } else {
            "Monitor Output".to_string()
        };

        let text = ratatui::text::Text::from(display_lines);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(title)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => match key.code {
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                    true
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    true
                }
                _ => false,
            },
            AppEvent::DataUpdated(_, data) => {
                self.update_output(data.clone());
                true
            }
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Diff Pane Implementation
pub struct DiffPane {
    visible: bool,
}

impl Default for DiffPane {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffPane {
    pub fn new() -> Self {
        Self { visible: true }
    }
}

impl Pane for DiffPane {
    fn title(&self) -> String {
        "Diff".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

        let theme = app.get_theme();
        if let Some(file) = app.get_current_file() {
            let file_path = file.path.to_string_lossy();
            let title = format!("Diff: {file_path}");

            let mut lines = Vec::new();

            for (i, line) in file.line_strings.iter().enumerate() {
                if i < app.get_scroll_offset() {
                    continue;
                }

                if lines.len() >= app.current_diff_height {
                    break;
                }

                let (style, line_text) = if line.starts_with('+') {
                    (Style::default().fg(theme.added_color()), line)
                } else if line.starts_with('-') {
                    (Style::default().fg(theme.removed_color()), line)
                } else if line.starts_with(' ') {
                    (Style::default().fg(theme.unchanged_color()), line)
                } else {
                    (Style::default().fg(theme.foreground_color()), line)
                };

                let span = Span::styled(line_text.clone(), style);
                lines.push(Line::from(span));
            }

            let text = ratatui::text::Text::from(lines);
            let paragraph = Paragraph::new(text)
                .block(
                    Block::default()
                        .title(title)
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(paragraph, area);
        } else {
            let paragraph = Paragraph::new("No changes detected").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
        }
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => matches!(key.code, KeyCode::Char('j') | KeyCode::Char('k')),
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Side-by-side Diff Pane Implementation
pub struct SideBySideDiffPane {
    visible: bool,
}

impl Default for SideBySideDiffPane {
    fn default() -> Self {
        Self::new()
    }
}

impl SideBySideDiffPane {
    pub fn new() -> Self {
        Self { visible: false }
    }
}

impl Pane for SideBySideDiffPane {
    fn title(&self) -> String {
        "Side-by-side Diff".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::Style,
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

        let theme = app.get_theme();
        if let Some(file) = app.get_current_file() {
            let file_path = file.path.to_string_lossy();
            let _title = format!("Side-by-side Diff: {file_path}");

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(area);

            let mut left_lines = Vec::new();
            let mut right_lines = Vec::new();

            let mut line_count = 0;
            for (i, line) in file.line_strings.iter().enumerate() {
                if i < app.get_scroll_offset() {
                    continue;
                }

                if line_count >= app.current_diff_height {
                    break;
                }

                let (left_content, right_content) = if let Some(stripped) = line.strip_prefix('+') {
                    ("".to_string(), stripped.to_string())
                } else if let Some(stripped) = line.strip_prefix('-') {
                    (stripped.to_string(), "".to_string())
                } else if let Some(stripped) = line.strip_prefix(' ') {
                    let content = stripped.to_string();
                    (content.clone(), content)
                } else {
                    (line.to_string(), line.to_string())
                };

                let left_style = if line.starts_with('-') {
                    Style::default().fg(theme.removed_color())
                } else if line.starts_with(' ') || line.starts_with('+') {
                    Style::default().fg(theme.unchanged_color())
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                let right_style = if line.starts_with('+') {
                    Style::default().fg(theme.added_color())
                } else if line.starts_with(' ') || line.starts_with('-') {
                    Style::default().fg(theme.unchanged_color())
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                left_lines.push(Line::from(Span::styled(left_content, left_style)));
                right_lines.push(Line::from(Span::styled(right_content, right_style)));

                line_count += 1;
            }

            let left_text = ratatui::text::Text::from(left_lines);
            let right_text = ratatui::text::Text::from(right_lines);

            let left_paragraph = Paragraph::new(left_text)
                .block(
                    Block::default()
                        .title("Original")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            let right_paragraph = Paragraph::new(right_text)
                .block(
                    Block::default()
                        .title("Modified")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(left_paragraph, chunks[0]);
            f.render_widget(right_paragraph, chunks[1]);
        } else {
            let paragraph = Paragraph::new("No changes detected").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
        }
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => matches!(key.code, KeyCode::Char('j') | KeyCode::Char('k')),
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Help Pane Implementation
pub struct HelpPane {
    visible: bool,
}

impl Default for HelpPane {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpPane {
    pub fn new() -> Self {
        Self { visible: false }
    }
}

impl Pane for HelpPane {
    fn title(&self) -> String {
        "Help".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, Paragraph, Wrap},
        };

        let theme = app.get_theme();
        let last_active_pane = app.get_last_active_pane();

        let mut help_text = vec![
            Line::from(Span::styled(
                "Git Repository Watcher - Help",
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];

        // Check if we're in commit picker mode and show commit picker shortcuts
        let (pane_title, pane_hotkeys) = if app.is_in_commit_picker_mode() {
            (
                "Commit Picker",
                vec![
                    "  j / k / ↑ / ↓     - Navigate commits",
                    "  g t               - Next commit",
                    "  g T               - Previous commit",
                    "  Enter             - Select commit",
                    "  Esc               - Exit commit picker",
                    "  Ctrl+P            - Enter commit picker mode",
                    "  Ctrl+W            - Return to working directory",
                ],
            )
        } else {
            match last_active_pane {
                ActivePane::FileTree => (
                    "File Tree",
                    vec![
                        "  Tab / g t     - Next file",
                        "  Shift+Tab / g T - Previous file",
                    ],
                ),
                ActivePane::Monitor => (
                    "Monitor",
                    vec![
                        "  Alt+j / Alt+Down  - Scroll down",
                        "  Alt+k / Alt+Up    - Scroll up",
                    ],
                ),
                ActivePane::Diff | ActivePane::SideBySideDiff => (
                    "Diff View",
                    vec![
                        "  j / Down / Ctrl+e - Scroll down",
                        "  k / Up / Ctrl+y   - Scroll up",
                        "  PageDown          - Page down",
                        "  PageUp            - Page up",
                        "  g g               - Go to top",
                        "  Shift+G           - Go to bottom",
                    ],
                ),
            }
        };

        help_text.push(Line::from(Span::styled(
            format!("{pane_title} Hotkeys:"),
            Style::default()
                .fg(theme.primary_color())
                .add_modifier(Modifier::BOLD),
        )));
        for hotkey in pane_hotkeys {
            help_text.push(Line::from(hotkey));
        }
        help_text.push(Line::from(""));

        help_text.extend(vec![
            Line::from(Span::styled(
                "General:",
                Style::default()
                    .fg(theme.primary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  ?             - Show/hide this help page"),
            Line::from("  Esc           - Exit help page"),
            Line::from("  Ctrl+h        - Toggle diff panel visibility"),
            Line::from("  Ctrl+o        - Toggle monitor pane visibility"),
            Line::from("  Ctrl+t        - Toggle light/dark theme"),
            Line::from("  q / Ctrl+c    - Quit application"),
        ]);

        // Add commit picker shortcut if not already in commit picker mode
        if !app.is_in_commit_picker_mode() {
            help_text.push(Line::from("  Ctrl+P        - Enter commit picker mode"));
        }

        // Add working directory shortcut if we have a selected commit
        if app.get_selected_commit().is_some() {
            help_text.push(Line::from("  Ctrl+W        - Return to working directory"));
        }

        help_text.extend(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Pane Modes:",
                Style::default()
                    .fg(theme.primary_color())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from("  Ctrl+d        - Switch to inline diff view"),
            Line::from("  Ctrl+s        - Switch to side-by-side diff view"),
            Line::from(""),
            Line::from("Press ? or Esc to return to the previous pane"),
        ]);

        let text = ratatui::text::Text::from(help_text);
        let paragraph = Paragraph::new(text)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => match key.code {
                KeyCode::Char('?') => {
                    self.set_visible(false);
                    true
                }
                KeyCode::Esc => {
                    self.set_visible(false);
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[cfg(test)]
mod help_tests {
    use super::*;
    use crate::ui::{App, Theme};
    use std::sync::Arc;

    fn create_test_llm_state() -> Arc<crate::shared_state::LlmSharedState> {
        Arc::new(crate::shared_state::LlmSharedState::new())
    }
    use crate::git::CommitInfo;

    #[test]
    fn test_help_detects_commit_picker_mode() {
        let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

        // Test normal mode
        assert!(!app.is_in_commit_picker_mode());

        // Enter commit picker mode
        app.enter_commit_picker_mode();
        assert!(app.is_in_commit_picker_mode());

        // Exit commit picker mode
        app.exit_commit_picker_mode();
        assert!(!app.is_in_commit_picker_mode());
    }

    #[test]
    fn test_help_detects_selected_commit() {
        let mut app = App::new_with_config(true, true, Theme::Dark, None, create_test_llm_state());

        // Initially no commit selected
        assert!(app.get_selected_commit().is_none());

        // Create a test commit and select it
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };
        app.select_commit(test_commit);

        // Now should have a selected commit
        assert!(app.get_selected_commit().is_some());

        // Clear the selected commit
        app.clear_selected_commit();
        assert!(app.get_selected_commit().is_none());
    }

    #[test]
    fn test_commit_summary_pane_cached_summary() {
        let mut pane = CommitSummaryPane::new_with_llm_client(None);

        // Create a test commit
        let test_commit = CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![],
        };

        // Update with commit
        pane.update_commit(Some(test_commit));

        // Initially should need summary
        assert!(pane.needs_summary());
        assert!(pane.llm_summary.is_none());

        // Set a cached summary
        pane.set_cached_summary("abc123", "This is a cached summary".to_string());

        // Should no longer need summary and should have the cached one
        assert!(!pane.needs_summary());
        assert_eq!(
            pane.llm_summary,
            Some("This is a cached summary".to_string())
        );
        assert_eq!(pane.loading_state, CommitSummaryLoadingState::Loaded);
    }
}

// Status Bar Pane Implementation
pub struct StatusBarPane {
    visible: bool,
}

impl Default for StatusBarPane {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusBarPane {
    pub fn new() -> Self {
        Self { visible: true }
    }
}

impl Pane for StatusBarPane {
    fn title(&self) -> String {
        "".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();
        let repo_name = &git_repo.repo_name;
        let branch = &git_repo.branch_name;
        let (commit_sha, commit_summary) = &git_repo.commit_info;
        let (total_files, total_additions, total_deletions) = git_repo.total_stats;
        let view_mode = git_repo.current_view_mode;

        let view_mode_text = if let Some(selected_commit) = app.get_selected_commit() {
            format!("🔍 Selected Commit: {}", selected_commit.short_sha)
        } else {
            match view_mode {
                crate::git::ViewMode::WorkingTree => "💼 Working Tree".to_string(),
                crate::git::ViewMode::Staged => "📋 Staged Files".to_string(),
                crate::git::ViewMode::DirtyDirectory => "🗂️ Dirty Directory".to_string(),
                crate::git::ViewMode::LastCommit => "📜 Last Commit".to_string(),
            }
        };

        let status_text = if let Some(selected_commit) = app.get_selected_commit() {
            format!(
                "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {} > {} | 📊 {} files (+{}/-{}) | Press Ctrl+W to return to working directory",
                selected_commit.short_sha,
                selected_commit.message.lines().next().unwrap_or(""),
                selected_commit.files_changed.len(),
                selected_commit
                    .files_changed
                    .iter()
                    .map(|f| f.additions)
                    .sum::<usize>(),
                selected_commit
                    .files_changed
                    .iter()
                    .map(|f| f.deletions)
                    .sum::<usize>()
            )
        } else {
            format!(
                "📂 {repo_name} | 🌿 {branch} | {view_mode_text} | 🎯 {commit_sha} > {commit_summary} | 📊 {total_files} files (+{total_additions}/-{total_deletions})"
            )
        };

        let paragraph = Paragraph::new(status_text)
            .style(
                Style::default()
                    .fg(theme.foreground_color())
                    .bg(theme.background_color())
                    .add_modifier(Modifier::REVERSED),
            )
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: false });

        f.render_widget(paragraph, area);
        Ok(())
    }

    fn handle_event(&mut self, _event: &AppEvent) -> bool {
        false
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

// Commit Picker Pane Implementation
pub struct CommitPickerPane {
    visible: bool,
    commits: Vec<crate::git::CommitInfo>,
    current_index: usize,
    scroll_offset: usize,
    last_g_press: Option<std::time::Instant>,
    enter_pressed: bool,
    loading_state: CommitPickerLoadingState,
    error_message: Option<String>,
    // Performance optimization fields
    last_visible_height: usize,
    render_cache_valid: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitPickerLoadingState {
    NotLoaded,
    Loading,
    Loaded,
    Error,
}

impl Default for CommitPickerPane {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitPickerPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            commits: Vec::new(),
            current_index: 0,
            scroll_offset: 0,
            last_g_press: None,
            enter_pressed: false,
            loading_state: CommitPickerLoadingState::NotLoaded,
            error_message: None,
            last_visible_height: 0,
            render_cache_valid: false,
        }
    }

    pub fn set_loading(&mut self) {
        self.loading_state = CommitPickerLoadingState::Loading;
        self.error_message = None;
    }

    pub fn update_commits(&mut self, commits: Vec<crate::git::CommitInfo>) {
        self.commits = commits;
        if self.current_index >= self.commits.len() {
            self.current_index = 0;
            self.scroll_offset = 0;
        }

        // Update loading state based on results
        if self.commits.is_empty() {
            self.loading_state = CommitPickerLoadingState::Loaded;
            // Don't set error for empty repos, just show appropriate message
        } else {
            self.loading_state = CommitPickerLoadingState::Loaded;
            self.error_message = None;
        }

        // Invalidate render cache when commits change
        self.render_cache_valid = false;
    }

    #[allow(dead_code)]
    pub fn set_error(&mut self, error: String) {
        self.loading_state = CommitPickerLoadingState::Error;
        self.error_message = Some(error);
        self.commits.clear();
        self.current_index = 0;
        self.scroll_offset = 0;
    }

    #[allow(dead_code)]
    pub fn is_loading(&self) -> bool {
        matches!(self.loading_state, CommitPickerLoadingState::Loading)
    }

    #[allow(dead_code)]
    pub fn has_error(&self) -> bool {
        matches!(self.loading_state, CommitPickerLoadingState::Error)
    }

    pub fn get_current_commit(&self) -> Option<&crate::git::CommitInfo> {
        // Only return commit if we're in a valid state
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.commits.get(self.current_index)
        } else {
            None
        }
    }

    fn navigate_next(&mut self) {
        // Only allow navigation if commits are loaded and available
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.current_index = (self.current_index + 1) % self.commits.len();
            self.update_scroll_offset(20); // Use reasonable default
        }
    }

    fn navigate_prev(&mut self) {
        // Only allow navigation if commits are loaded and available
        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
            && !self.commits.is_empty()
        {
            self.current_index = if self.current_index == 0 {
                self.commits.len() - 1
            } else {
                self.current_index - 1
            };
            self.update_scroll_offset(20); // Use reasonable default
        }
    }

    fn update_scroll_offset(&mut self, visible_height: usize) {
        // Ensure current selection is visible
        if self.current_index < self.scroll_offset {
            self.scroll_offset = self.current_index;
            self.render_cache_valid = false;
        } else if self.current_index >= self.scroll_offset + visible_height {
            self.scroll_offset = self.current_index.saturating_sub(visible_height - 1);
            self.render_cache_valid = false;
        }

        // Update last visible height for performance tracking
        if self.last_visible_height != visible_height {
            self.last_visible_height = visible_height;
            self.render_cache_valid = false;
        }
    }

    pub fn is_enter_pressed(&self) -> bool {
        self.enter_pressed
    }

    pub fn reset_enter_pressed(&mut self) {
        self.enter_pressed = false;
    }

    pub fn get_commits(&self) -> Vec<crate::git::CommitInfo> {
        self.commits.clone()
    }

    pub fn get_current_index(&self) -> usize {
        self.current_index
    }
}

impl Pane for CommitPickerPane {
    fn title(&self) -> String {
        "Commit History".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, List, ListItem, Paragraph},
        };

        let theme = app.get_theme();

        // Handle different loading states
        match self.loading_state {
            CommitPickerLoadingState::NotLoaded => {
                let paragraph = Paragraph::new("Press Ctrl+P to load commit history").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Loading => {
                let paragraph = Paragraph::new("⏳ Loading commit history...").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Error => {
                let error_text = if let Some(error) = &self.error_message {
                    format!("❌ Error loading commits:\n{}", error)
                } else {
                    "❌ Error loading commits".to_string()
                };

                let paragraph = Paragraph::new(error_text)
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }
            CommitPickerLoadingState::Loaded => {
                if self.commits.is_empty() {
                    let paragraph = Paragraph::new("📭 No commits found in this repository\n\nThis might be a new repository with no commits yet.")
                        .block(
                            Block::default()
                                .title(self.title())
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(theme.border_color())),
                        )
                        .style(Style::default().fg(theme.secondary_color()));
                    f.render_widget(paragraph, area);
                    return Ok(());
                }
            }
        }

        // Calculate visible range based on scroll offset and area height
        let visible_height = area.height.saturating_sub(2) as usize; // Account for borders
        let start_index = self.scroll_offset;
        let end_index = (start_index + visible_height).min(self.commits.len());

        // Early return if we have no commits to render
        if start_index >= self.commits.len() {
            let paragraph = Paragraph::new("No commits to display").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
            return Ok(());
        }

        let commit_items: Vec<ListItem> = self
            .commits
            .iter()
            .enumerate()
            .skip(start_index)
            .take(end_index - start_index)
            .map(|(original_index, commit)| {
                let mut spans = Vec::new();

                // Add arrow for current selection (use original index for comparison)
                if original_index == self.current_index {
                    spans.push(Span::styled(
                        "-> ",
                        Style::default()
                            .fg(theme.secondary_color())
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    spans.push(Span::raw("   "));
                }

                // Add short SHA
                spans.push(Span::styled(
                    format!("{} ", commit.short_sha),
                    Style::default()
                        .fg(theme.primary_color())
                        .add_modifier(Modifier::BOLD),
                ));

                // Add first line of commit message
                let first_line = commit.message.lines().next().unwrap_or("").to_string();
                spans.push(Span::styled(
                    first_line,
                    Style::default().fg(theme.foreground_color()),
                ));

                let line_style = if original_index == self.current_index {
                    Style::default()
                        .fg(theme.foreground_color())
                        .bg(theme.highlight_color())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.foreground_color())
                };

                let line = Line::from(spans).style(line_style);
                ListItem::new(line)
            })
            .collect();

        let commit_list = List::new(commit_items)
            .block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            )
            .highlight_style(
                Style::default()
                    .fg(theme.secondary_color())
                    .add_modifier(Modifier::BOLD),
            );

        f.render_widget(commit_list, area);
        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => {
                match key.code {
                    KeyCode::Char('j') => {
                        self.navigate_next();
                        true
                    }
                    KeyCode::Char('k') => {
                        self.navigate_prev();
                        true
                    }
                    KeyCode::Down => {
                        self.navigate_next();
                        true
                    }
                    KeyCode::Up => {
                        self.navigate_prev();
                        true
                    }
                    KeyCode::Char('g') => {
                        self.last_g_press = Some(std::time::Instant::now());
                        true
                    }
                    KeyCode::Char('t') => {
                        // Check if g was pressed recently for g+t navigation
                        if let Some(last_time) = self.last_g_press
                            && std::time::Instant::now()
                                .duration_since(last_time)
                                .as_millis()
                                < 500
                        {
                            self.navigate_next();
                            self.last_g_press = None; // Reset after use
                        }
                        true
                    }
                    KeyCode::Char('T') => {
                        // Check if g was pressed recently for g+T navigation
                        if let Some(last_time) = self.last_g_press
                            && std::time::Instant::now()
                                .duration_since(last_time)
                                .as_millis()
                                < 500
                        {
                            self.navigate_prev();
                            self.last_g_press = None; // Reset after use
                        }
                        true
                    }
                    KeyCode::Enter => {
                        // Only allow commit selection if we have valid commits loaded
                        if matches!(self.loading_state, CommitPickerLoadingState::Loaded)
                            && !self.commits.is_empty()
                            && self.current_index < self.commits.len()
                        {
                            self.enter_pressed = true;
                        }
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn as_commit_picker_pane(&self) -> Option<&CommitPickerPane> {
        Some(self)
    }

    fn as_commit_picker_pane_mut(&mut self) -> Option<&mut CommitPickerPane> {
        Some(self)
    }
}

// Commit Summary Pane Implementation
pub struct CommitSummaryPane {
    visible: bool,
    current_commit: Option<crate::git::CommitInfo>,
    scroll_offset: usize,
    llm_summary: Option<String>,
    llm_client: Option<LlmClient>,
    is_loading_summary: bool,
    pending_summary_sha: Option<String>, // Track which commit we're waiting for a summary for
    llm_shared_state: Option<Arc<LlmSharedState>>,
    loading_state: CommitSummaryLoadingState,
    cache_callback: Option<(String, String)>, // (commit_sha, summary) to cache
}

#[derive(Debug, Clone, PartialEq)]
pub enum CommitSummaryLoadingState {
    NoCommit,
    #[allow(dead_code)]
    LoadingSummary,
    Loaded,
    #[allow(dead_code)]
    Error,
}

impl Default for CommitSummaryPane {
    fn default() -> Self {
        Self::new()
    }
}

impl CommitSummaryPane {
    pub fn new() -> Self {
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client: None,
            is_loading_summary: false,
            pending_summary_sha: None,
            llm_shared_state: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
            cache_callback: None,
        }
    }

    pub fn new_with_llm_client(llm_client: Option<LlmClient>) -> Self {
        Self {
            visible: false,
            current_commit: None,
            scroll_offset: 0,
            llm_summary: None,
            llm_client,
            is_loading_summary: false,
            pending_summary_sha: None,
            llm_shared_state: None,
            loading_state: CommitSummaryLoadingState::NoCommit,
            cache_callback: None,
        }
    }

    pub fn update_commit(&mut self, commit: Option<crate::git::CommitInfo>) {
        let commit_changed = match (&self.current_commit, &commit) {
            (Some(old), Some(new)) => old.sha != new.sha,
            (None, Some(_)) => true,
            (Some(_), None) => true,
            (None, None) => false,
        };

        self.current_commit = commit;

        if commit_changed {
            // Reset state when commit changes
            self.llm_summary = None;
            self.scroll_offset = 0;
            self.is_loading_summary = false;
            self.pending_summary_sha = None;
            self.clear_error();
            self.cache_callback = None;

            // Update loading state based on new commit
            if self.current_commit.is_some() {
                // Since commits from get_commit_history already have files_changed populated,
                // we can immediately show the files and only wait for LLM summary
                self.loading_state = CommitSummaryLoadingState::Loaded;
                // Don't request LLM summary immediately - let the App check cache first
            } else {
                self.loading_state = CommitSummaryLoadingState::NoCommit;
            }
        }
    }

    pub fn set_shared_state(&mut self, llm_shared_state: Arc<LlmSharedState>) {
        self.llm_shared_state = Some(llm_shared_state);
    }

    #[allow(dead_code)]
    pub fn set_error(&mut self, error: String) {
        self.loading_state = CommitSummaryLoadingState::Error;
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.set_error("commit_summary".to_string(), error);
        }
        self.is_loading_summary = false;
        self.pending_summary_sha = None;
    }

    pub fn clear_error(&mut self) {
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.clear_error("commit_summary");
        }
    }

    pub fn get_error(&self) -> Option<String> {
        if let Some(shared_state) = &self.llm_shared_state {
            shared_state.get_error("commit_summary")
        } else {
            None
        }
    }

    fn request_llm_summary(&mut self) {
        if let Some(_commit) = &self.current_commit {
            self.loading_state = CommitSummaryLoadingState::Loaded;
            self.llm_summary =
                Some("LLM summary generation not yet implemented with shared state".to_string());
        }
    }

    #[allow(dead_code)]
    pub fn poll_llm_summary(&mut self) {
        // This method is now deprecated - use shared state instead
        // The actual summary polling is handled through shared state in the main loop
    }

    /// Set a cached summary directly without generating a new one
    pub fn set_cached_summary(&mut self, commit_sha: &str, summary: String) {
        if let Some(current_commit) = &self.current_commit
            && current_commit.sha == commit_sha
        {
            self.llm_summary = Some(summary);
            self.clear_error();
            self.is_loading_summary = false;
            self.pending_summary_sha = None;
            self.loading_state = CommitSummaryLoadingState::Loaded;
        }
    }

    /// Check if we need to request a summary for the current commit
    pub fn needs_summary(&self) -> bool {
        if let Some(_current_commit) = &self.current_commit {
            // Need summary if we don't have one and we're not currently loading
            self.llm_summary.is_none() && !self.is_loading_summary
        } else {
            false
        }
    }

    /// Get the current commit SHA if available
    #[allow(dead_code)]
    pub fn get_current_commit_sha(&self) -> Option<String> {
        self.current_commit.as_ref().map(|c| c.sha.clone())
    }

    /// Force generation of a new summary (bypassing cache)
    pub fn force_generate_summary(&mut self) {
        self.llm_summary = None;
        self.clear_error();
        self.request_llm_summary();
    }

    /// Get and clear any pending cache callback
    pub fn take_cache_callback(&mut self) -> Option<(String, String)> {
        self.cache_callback.take()
    }
}

impl Pane for CommitSummaryPane {
    fn title(&self) -> String {
        "Commit Details".to_string()
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::{Modifier, Style},
            text::{Line, Span},
            widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
        };

        let theme = app.get_theme();

        // Handle different loading states
        match self.loading_state {
            CommitSummaryLoadingState::NoCommit => {
                let paragraph = Paragraph::new("No commit selected").block(
                    Block::default()
                        .title(self.title())
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                );
                f.render_widget(paragraph, area);
                return Ok(());
            }

            CommitSummaryLoadingState::Error => {
                let error_text = if let Some(error) = self.get_error() {
                    format!("❌ Error loading commit details:\n{}", error)
                } else {
                    "❌ Error loading commit details".to_string()
                };

                let paragraph = Paragraph::new(error_text)
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }
            _ => {} // Continue with normal rendering for LoadingSummary and Loaded states
        }

        if let Some(commit) = &self.current_commit {
            // Validate commit data before rendering
            if commit.sha.is_empty() {
                let paragraph = Paragraph::new("❌ Invalid commit data")
                    .block(
                        Block::default()
                            .title(self.title())
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.error_color())),
                    )
                    .style(Style::default().fg(theme.error_color()));
                f.render_widget(paragraph, area);
                return Ok(());
            }

            // Split the area into two sections: file changes and LLM summary
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Percentage(70), Constraint::Percentage(30)])
                .split(area);

            // Render file changes section
            let mut file_items = Vec::new();

            if commit.files_changed.is_empty() {
                // Show message when no file changes are available
                file_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "ℹ️  No file changes detected",
                    Style::default().fg(theme.secondary_color()),
                )])));
                file_items.push(ListItem::new(Line::from(vec![Span::styled(
                    "   This might be a merge commit or there was an error parsing changes",
                    Style::default().fg(theme.foreground_color()),
                )])));
            } else {
                for (index, file_change) in commit.files_changed.iter().enumerate() {
                    if index < self.scroll_offset {
                        continue;
                    }

                    let visible_height = chunks[0].height.saturating_sub(2) as usize; // Account for borders
                    if file_items.len() >= visible_height {
                        break;
                    }

                    let mut spans = Vec::new();

                    // Status indicator with validation
                    let status_char = match file_change.status {
                        crate::git::FileChangeStatus::Added => "📄 ",
                        crate::git::FileChangeStatus::Modified => "📝 ",
                        crate::git::FileChangeStatus::Deleted => "🗑️  ",
                        crate::git::FileChangeStatus::Renamed => "📋 ",
                    };
                    spans.push(Span::raw(status_char));

                    // File path with length validation
                    let file_path_str = file_change.path.to_string_lossy();
                    let display_path = if file_path_str.len() > 80 {
                        format!("...{}", &file_path_str[file_path_str.len() - 77..])
                    } else {
                        file_path_str.to_string()
                    };

                    spans.push(Span::styled(
                        display_path,
                        Style::default().fg(theme.foreground_color()),
                    ));

                    // Addition/deletion counts with validation
                    if file_change.additions > 0 {
                        let additions_text = if file_change.additions > 9999 {
                            " (+9999+)".to_string()
                        } else {
                            format!(" (+{})", file_change.additions)
                        };
                        spans.push(Span::styled(
                            additions_text,
                            Style::default()
                                .fg(theme.added_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    if file_change.deletions > 0 {
                        let deletions_text = if file_change.deletions > 9999 {
                            " (-9999+)".to_string()
                        } else {
                            format!(" (-{})", file_change.deletions)
                        };
                        spans.push(Span::styled(
                            deletions_text,
                            Style::default()
                                .fg(theme.removed_color())
                                .add_modifier(Modifier::BOLD),
                        ));
                    }

                    let line = Line::from(spans);
                    file_items.push(ListItem::new(line));
                }
            }

            let file_list = List::new(file_items).block(
                Block::default()
                    .title(format!("Files Changed ({})", commit.files_changed.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );

            f.render_widget(file_list, chunks[0]);

            // Render LLM summary section with enhanced error handling and loading states
            let summary_content = if let Some(summary) = &self.llm_summary {
                summary.clone()
            } else if self.is_loading_summary {
                "⏳ Generating summary...".to_string()
            } else if self.llm_client.is_none() {
                "LLM client not available".to_string()
            } else {
                "📋 Checking cache...".to_string()
            };

            let summary_lines: Vec<Line> = summary_content
                .lines()
                .map(|line| Line::from(line.to_string()))
                .collect();

            let summary_paragraph = Paragraph::new(summary_lines)
                .block(
                    Block::default()
                        .title("LLM Summary")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(theme.border_color())),
                )
                .wrap(Wrap { trim: false });

            f.render_widget(summary_paragraph, chunks[1]);
        } else {
            // No commit selected
            let paragraph = Paragraph::new("No commit selected").block(
                Block::default()
                    .title(self.title())
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color())),
            );
            f.render_widget(paragraph, area);
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key) => {
                match key.code {
                    KeyCode::Char('j') | KeyCode::Down => {
                        if let Some(commit) = &self.current_commit {
                            let max_scroll = commit.files_changed.len().saturating_sub(1);
                            self.scroll_offset =
                                std::cmp::min(self.scroll_offset.saturating_add(1), max_scroll);
                        }
                        true
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        true
                    }
                    KeyCode::PageDown => {
                        if let Some(commit) = &self.current_commit {
                            let page_size = 10; // Approximate page size
                            let max_scroll = commit.files_changed.len().saturating_sub(page_size);
                            self.scroll_offset = std::cmp::min(
                                self.scroll_offset.saturating_add(page_size),
                                max_scroll,
                            );
                        }
                        true
                    }
                    KeyCode::PageUp => {
                        let page_size = 10; // Approximate page size
                        self.scroll_offset = self.scroll_offset.saturating_sub(page_size);
                        true
                    }
                    KeyCode::Char('g') => {
                        // Go to top
                        self.scroll_offset = 0;
                        true
                    }
                    KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        // Go to bottom
                        if let Some(commit) = &self.current_commit {
                            self.scroll_offset = commit.files_changed.len().saturating_sub(1);
                        }
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }

    fn as_commit_summary_pane(&self) -> Option<&CommitSummaryPane> {
        Some(self)
    }

    fn as_commit_summary_pane_mut(&mut self) -> Option<&mut CommitSummaryPane> {
        Some(self)
    }
}

impl Pane for AdvicePanel {
    fn title(&self) -> String {
        match self.mode {
            AdviceMode::Chatting => "Chat".to_string(),
            AdviceMode::Help => "Help".to_string(),
        }
    }

    fn render(
        &self,
        f: &mut Frame,
        app: &App,
        area: Rect,
        _git_repo: &GitRepo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let theme = app.get_theme();

        // Update the current diff content from the app's files
        let files = app.get_files();
        if !files.is_empty() {
            let mut diff_content = String::new();

            for file_diff in files {
                // Add file header
                diff_content.push_str(&format!("diff --git a/{} b/{}\n",
                    file_diff.path.to_string_lossy(),
                    file_diff.path.to_string_lossy()));

                // Add git index line based on status
                if file_diff.status.contains(git2::Status::WT_NEW) {
                    diff_content.push_str("new file mode 100644\n");
                } else if file_diff.status.contains(git2::Status::WT_DELETED) {
                    diff_content.push_str("deleted file mode 100644\n");
                } else if file_diff.status.contains(git2::Status::WT_RENAMED) {
                    diff_content.push_str("similarity index 100%\n");
                    diff_content.push_str("rename from old_name\n");
                    diff_content.push_str("rename to new_name\n");
                } else {
                    diff_content.push_str("index 0000000..1111111 100644\n");
                }

                // Add file change markers
                diff_content.push_str(&format!("--- a/{}\n", file_diff.path.to_string_lossy()));
                diff_content.push_str(&format!("+++ b/{}\n", file_diff.path.to_string_lossy()));

                // Add the actual diff content
                for line in &file_diff.line_strings {
                    diff_content.push_str(line);
                    diff_content.push('\n');
                }

                diff_content.push('\n'); // Add separator between files
            }

            *self.current_diff_content.borrow_mut() = if diff_content.trim().is_empty() {
                None
            } else {
                Some(diff_content)
            };
        } else {
            *self.current_diff_content.borrow_mut() = None;
        }

        // Check if we need to send the initial message (only on first visit)
        // Use a mutable reference to self for this operation
        if self.visible && !self.initial_message_sent && self.first_visit {
            // This is a workaround to call a method that requires &mut self from &self
            unsafe {
                let self_mut = self as *const AdvicePanel as *mut AdvicePanel;
                if let Some(diff_content) = (*self_mut).current_diff_content.borrow().as_ref() {
                    if !diff_content.is_empty() {
                        (*self_mut).send_initial_message_with_diff(diff_content);
                        (*self_mut).initial_message_sent = true;
                        (*self_mut).first_visit = false;
                    } else if files.is_empty() {
                        // No files available, send message about no changes
                        (*self_mut).send_no_changes_message();
                        (*self_mut).initial_message_sent = true;
                        (*self_mut).first_visit = false;
                    }
                } else if files.is_empty() {
                    // No files available, send message about no changes
                    (*self_mut).send_no_changes_message();
                    (*self_mut).initial_message_sent = true;
                    (*self_mut).first_visit = false;
                }
            }
        }

        let block = Block::default()
            .title(self.title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_color()));

        let content = match &self.content {
            AdviceContent::Loading => {
                vec![Line::from("Loading advice...".to_string())]
            }
            AdviceContent::Error(error) => {
                vec![Line::from(format!("Error: {}", error))]
            }
            AdviceContent::Improvements(improvements) => {
                let mut lines = Vec::new();
                for imp in improvements {
                    // Use chat-style timestamp format
                    let time_str = "now"; // Improvements are always current
                    lines.push(Line::from(format!("[{}] AI:", time_str)).fg(Color::Green));

                    // Add title as the main message
                    for title_line in imp.title.lines() {
                        if !title_line.trim().is_empty() {
                            lines.push(Line::from(format!("  {}", title_line)));
                        }
                    }

                    // Add description with chat-style indentation
                    for desc_line in imp.description.lines() {
                        if !desc_line.trim().is_empty() {
                            lines.push(Line::from(format!("  {}", desc_line)));
                        }
                    }

                    // Add category and priority in a subtle way
                    let priority_emoji = match imp.priority {
                        ImprovementPriority::Low => "🟢",
                        ImprovementPriority::Medium => "🟡",
                        ImprovementPriority::High => "🟠",
                        ImprovementPriority::Critical => "🔴",
                        ImprovementPriority::Unknown => "⚪",
                    };
                    lines.push(Line::from(format!("  {} {} · {}", priority_emoji, imp.category, imp.priority)));
                    lines.push(Line::from("")); // Empty line between messages
                }
                lines
            }
            AdviceContent::Chat(messages) => {
                let mut lines = Vec::new();
                for msg in messages {
                    // Skip user messages that contain the diff pattern (initial automated message)
                    if msg.role == MessageRole::User && msg.content.contains("Please provide 3 actionable improvements for the following code changes:") {
                        continue;
                    }
                    let (prefix, color) = match msg.role {
                        MessageRole::User => ("You", Color::Cyan),
                        MessageRole::Assistant => ("AI", Color::Green),
                        MessageRole::System => ("System", Color::Yellow),
                    };

                    // Add message header with timestamp
                    let time_str = msg.timestamp.elapsed().unwrap_or_default().as_secs();
                    let time_display = if time_str < 60 {
                        format!("{}s ago", time_str)
                    } else if time_str < 3600 {
                        format!("{}m ago", time_str / 60)
                    } else {
                        format!("{}h ago", time_str / 3600)
                    };

                    lines.push(Line::from(format!("[{}] {}:", time_display, prefix)).fg(color));

                    // Add message content with line wrapping
                    for content_line in msg.content.lines() {
                        if !content_line.trim().is_empty() {
                            lines.push(Line::from(format!("  {}", content_line)));
                        }
                    }
                    lines.push(Line::from("")); // Empty line between messages
                }

                // Add "thinking" indicator if waiting for AI response
                if self.loading_state == LoadingState::SendingChat && self.pending_chat_message_id.is_some() {
                    lines.push(Line::from("[now] AI:").fg(Color::Green));
                    lines.push(Line::from("  🤔 Thinking...".to_string()).fg(Color::Yellow));
                    lines.push(Line::from(""));
                }

                lines
            }
            AdviceContent::Help(help_text) => {
                help_text.lines().map(Line::from).collect()
            }
        };

        // Adjust content area when chat input is active to make room for chat input
        let content_area = if self.mode == AdviceMode::Chatting && self.chat_input_active {
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: area.height.saturating_sub(3), // Reserve space for chat input
            }
        } else {
            area
        };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(paragraph, content_area);

        // Show chat input only when activated
        if self.mode == AdviceMode::Chatting && self.chat_input_active {
            let input_area = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(3),
                width: area.width,
                height: 3,
            };

            let input_block = Block::default()
                .title("Chat Input")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.border_color()));

            let input_text = format!("> {}", self.chat_input);
            let input_paragraph = Paragraph::new(input_text)
                .block(input_block)
                .wrap(Wrap { trim: false });

            f.render_widget(input_paragraph, input_area);
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key_event) => {
                match self.mode {
                    AdviceMode::Chatting => {
                        if self.chat_input_active {
                            // Chat input is active, handle input keys
                            match key_event.code {
                                KeyCode::Enter => {
                                    if !self.chat_input.is_empty() {
                                        let message = self.chat_input.clone();
                                        self.chat_input.clear();
                                        self.chat_input_active = false;
                                        // Send the message
                                        let _ = self.send_chat_message(&message);
                                    }
                                    true
                                }
                                KeyCode::Esc => {
                                    self.chat_input_active = false;
                                    self.chat_input.clear();
                                    true
                                }
                                KeyCode::Char(c) => {
                                    self.chat_input.push(c);
                                    true
                                }
                                KeyCode::Backspace => {
                                    self.chat_input.pop();
                                    true
                                }
                                _ => false,
                            }
                        } else {
                            // Chat input is not active, handle navigation and activation keys
                            match key_event.code {
                                KeyCode::Char('/') => {
                                    self.chat_input_active = true;
                                    true
                                }
                                KeyCode::Char('?') => {
                                    self.mode = AdviceMode::Help;
                                    // Reset scroll offset when entering help mode
                                    self.scroll_offset = 0;
                                    // Backup current chat content before switching to help
                                    self.chat_content_backup = Some(self.content.clone());
                                    // Set help content when entering help mode
                                    let help_text = vec![
                                        "Git Repository Watcher - Chat Interface Help",
                                        "",
                                        "Navigation:",
                                        "  j / k / ↑ / ↓     - Scroll up/down",
                                        "  PageUp / PageDown  - Scroll faster",
                                        "  g                  - Go to top",
                                        "  Shift+G            - Go to bottom",
                                        "",
                                        "Chat Interface:",
                                        "  /                  - Activate chat input",
                                        "  Enter              - Send message (when input active)",
                                        "  Esc                - Deactivate chat input",
                                        "",
                                        "Other Controls:",
                                        "  Ctrl+R             - Refresh diff and clear chat",
                                        "  Esc                - Exit advice pane",
                                        "  ?                  - Show this help",
                                        "",
                                        "Tips:",
                                        "- Chat history is preserved across panel activations",
                                        "- Initial message with diff is sent automatically on first visit",
                                        "- Use Ctrl+R to refresh with latest diff and start fresh conversation",
                                    ].join("\n");
                                    self.content = AdviceContent::Help(help_text);
                                    true
                                }
                                KeyCode::Esc => {
                                    if self.chat_input_active {
                                        // Deactivate chat input
                                        self.chat_input_active = false;
                                        self.chat_input.clear();
                                        true
                                    } else {
                                        false // Let parent handle Esc for panel closing
                                    }
                                }
                                // Navigation keys - always work when chat input is inactive
                                KeyCode::Char('j') | KeyCode::Down => {
                                    self.scroll_offset = self.scroll_offset.saturating_add(1);
                                    true
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    self.scroll_offset = self.scroll_offset.saturating_sub(1);
                                    true
                                }
                                KeyCode::PageDown => {
                                    self.scroll_offset = self.scroll_offset.saturating_add(10);
                                    true
                                }
                                KeyCode::PageUp => {
                                    self.scroll_offset = self.scroll_offset.saturating_sub(10);
                                    true
                                }
                                KeyCode::Char('g') => {
                                    self.scroll_offset = 0;
                                    true
                                }
                                KeyCode::Char('G') if key_event.modifiers.contains(KeyModifiers::SHIFT) => {
                                    // Scroll to actual bottom by calculating content height
                                    let content_lines = match &self.content {
                                        AdviceContent::Chat(messages) => {
                                            let mut line_count = 0;
                                            for msg in messages {
                                                // Skip user messages that contain the diff pattern
                                                if msg.role == MessageRole::User && msg.content.contains("Please provide 3 actionable improvements for the following code changes:") {
                                                    continue;
                                                }
                                                // Count header line
                                                line_count += 1;
                                                // Count content lines
                                                line_count += msg.content.lines().count();
                                                // Add empty line between messages
                                                line_count += 1;
                                            }
                                            // Add thinking indicator if present
                                            if self.loading_state == LoadingState::SendingChat && self.pending_chat_message_id.is_some() {
                                                line_count += 3; // "AI:" + "Thinking..." + empty line
                                            }
                                            line_count
                                        }
                                        AdviceContent::Help(help_text) => help_text.lines().count(),
                                        _ => 0,
                                    };
                                    let visible_lines = 20; // Approximate visible area height
                                    self.scroll_offset = content_lines.saturating_sub(visible_lines).max(0);
                                    true
                                }
                                KeyCode::Char('r') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                                    // Ctrl+R: Refresh diff and clear chat
                                    self.refresh_chat_with_new_diff();
                                    true
                                }
                                _ => false,
                            }
                        }
                    }
                    AdviceMode::Help => {
                        match key_event.code {
                            KeyCode::Esc => {
                                // Exit help mode and restore chat content
                                self.mode = AdviceMode::Chatting;
                                // Restore backed up chat content if available
                                if let Some(backup_content) = self.chat_content_backup.take() {
                                    self.content = backup_content;
                                } else {
                                    // Fallback to empty chat if no backup
                                    self.content = AdviceContent::Chat(Vec::new());
                                }
                                false // Let parent handle Esc for panel closing
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                self.scroll_offset = self.scroll_offset.saturating_add(1);
                                true
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                                true
                            }
                            _ => false,
                        }
                    }
                }
            }
            _ => false,
        }
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn set_visible(&mut self, visible: bool) {
        let was_visible = self.visible;
        self.visible = visible;

        // When panel becomes visible, set up chat input state but preserve history
        if visible && !was_visible {
            debug!("🎯 ADVICE_PANEL: Panel became visible");
            self.chat_input_active = false;
        }
    }

    fn as_advice_pane(&self) -> Option<&AdvicePanel> {
        Some(self)
    }

    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePanel> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AdviceConfig, Config};

    #[test]
    fn test_advice_panel_creation() {
        // Test that AdvicePanel can be created with default configuration
        let config = Config::default();
        let advice_config = AdviceConfig::default();

        let panel = AdvicePanel::new(config, advice_config);

        assert!(panel.is_ok(), "Failed to create AdvicePanel");

        let panel = panel.unwrap();
        assert!(!panel.visible()); // Should be hidden by default
        assert_eq!(panel.get_mode(), AdviceMode::Viewing);
        assert!(panel.get_improvements().is_empty());
        assert!(panel.get_chat_history().is_empty());
    }

    #[test]
    fn test_advice_panel_visibility() {
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Initially hidden (not default pane)
        assert!(!panel.visible());

        // Toggle to show
        panel.toggle_visibility();
        assert!(panel.visible());

        // Toggle to hide
        panel.toggle_visibility();
        assert!(!panel.visible());

        // Set explicitly
        panel.set_visibility(true);
        assert!(panel.visible());
        panel.set_visibility(false);
        assert!(!panel.visible());
    }

    #[test]
    fn test_advice_panel_modes() {
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Start in viewing mode
        assert_eq!(panel.get_mode(), AdviceMode::Viewing);

        // Test key events for mode switching
        let enter_chat = AppEvent::Key(KeyEvent::new(
            KeyCode::Char('/'),
            KeyModifiers::NONE,
        ));

        let handled = panel.handle_event(&enter_chat);
        assert!(handled, "Should handle entering chat mode");
        assert_eq!(panel.get_mode(), AdviceMode::Chatting);

        let exit_chat = AppEvent::Key(KeyEvent::new(
            KeyCode::Esc,
            KeyModifiers::NONE,
        ));

        let handled = panel.handle_event(&exit_chat);
        assert!(handled, "Should handle exiting chat mode");
        assert_eq!(panel.get_mode(), AdviceMode::Viewing);
    }

    use crate::config::LlmConfig;
    use std::env;
    use std::sync::Arc;

    fn create_test_pane_registry() -> PaneRegistry {
        let mut llm_config = LlmConfig::default();
        if env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = LlmClient::new(llm_config).unwrap();
        let llm_shared_state = Arc::new(LlmSharedState::new());
        PaneRegistry::new(Theme::Dark, llm_client, llm_shared_state)
    }

    #[test]
    fn test_pane_registry_creation() {
        let registry = create_test_pane_registry();
        assert_eq!(registry.panes.len(), 9); // Default panes + commit picker + commit summary + advice pane
        assert!(registry.get_pane(&PaneId::FileTree).is_some());
        assert!(registry.get_pane(&PaneId::Monitor).is_some());
        assert!(registry.get_pane(&PaneId::Diff).is_some());
        assert!(registry.get_pane(&PaneId::CommitPicker).is_some());
        assert!(registry.get_pane(&PaneId::CommitSummary).is_some());
        assert!(registry.get_pane(&PaneId::Advice).is_some());
    }

    #[test]
    fn test_commit_picker_pane_navigation() {
        let mut pane = CommitPickerPane::new();

        // Test with empty commits
        assert_eq!(pane.current_index, 0);
        pane.navigate_next();
        assert_eq!(pane.current_index, 0);
        pane.navigate_prev();
        assert_eq!(pane.current_index, 0);

        // Add some test commits
        let commits = vec![
            crate::git::CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "First commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-01".to_string(),
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-02".to_string(),
                files_changed: vec![],
            },
        ];

        pane.update_commits(commits);

        // Test navigation
        assert_eq!(pane.current_index, 0);
        pane.navigate_next();
        assert_eq!(pane.current_index, 1);
        pane.navigate_next();
        assert_eq!(pane.current_index, 0); // Should wrap around

        pane.navigate_prev();
        assert_eq!(pane.current_index, 1); // Should wrap around backwards
        pane.navigate_prev();
        assert_eq!(pane.current_index, 0);
    }

    #[test]
    fn test_commit_picker_pane_key_handling() {
        let mut pane = CommitPickerPane::new();

        // Add test commits
        let commits = vec![
            crate::git::CommitInfo {
                sha: "abc123".to_string(),
                short_sha: "abc123".to_string(),
                message: "First commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-01".to_string(),
                files_changed: vec![],
            },
            crate::git::CommitInfo {
                sha: "def456".to_string(),
                short_sha: "def456".to_string(),
                message: "Second commit".to_string(),
                author: "Test Author".to_string(),
                date: "2023-01-02".to_string(),
                files_changed: vec![],
            },
        ];

        pane.update_commits(commits);

        // Test j key (next)
        let j_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(pane.handle_event(&j_event));
        assert_eq!(pane.current_index, 1);

        // Test k key (prev)
        let k_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(pane.handle_event(&k_event));
        assert_eq!(pane.current_index, 0);

        // Test g+t combination
        let g_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(pane.handle_event(&g_event));

        // Immediately follow with t
        let t_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::NONE));
        assert!(pane.handle_event(&t_event));
        assert_eq!(pane.current_index, 1); // Should navigate next
    }

    #[test]
    fn test_commit_summary_pane_creation() {
        let pane = CommitSummaryPane::new();
        assert!(!pane.visible());
        assert!(pane.current_commit.is_none());
        assert_eq!(pane.scroll_offset, 0);
        assert!(pane.llm_summary.is_none());
    }

    #[test]
    fn test_commit_summary_pane_update_commit() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![crate::git::CommitFileChange {
                path: std::path::PathBuf::from("test.rs"),
                status: crate::git::FileChangeStatus::Modified,
                additions: 5,
                deletions: 2,
            }],
        };

        pane.update_commit(Some(commit.clone()));
        assert!(pane.current_commit.is_some());
        assert_eq!(pane.current_commit.as_ref().unwrap().sha, "abc123");
        assert_eq!(pane.scroll_offset, 0);
        assert!(pane.llm_summary.is_none());
    }

    #[test]
    fn test_commit_summary_pane_scrolling() {
        let mut pane = CommitSummaryPane::new();

        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: (0..20)
                .map(|i| crate::git::CommitFileChange {
                    path: std::path::PathBuf::from(format!("file{}.rs", i)),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: i,
                    deletions: i / 2,
                })
                .collect(),
        };

        pane.update_commit(Some(commit));

        // Test j key (scroll down)
        let j_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE));
        assert!(pane.handle_event(&j_event));
        assert_eq!(pane.scroll_offset, 1);

        // Test k key (scroll up)
        let k_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE));
        assert!(pane.handle_event(&k_event));
        assert_eq!(pane.scroll_offset, 0);

        // Test page down
        let page_down_event = AppEvent::Key(KeyEvent::from(KeyCode::PageDown));
        assert!(pane.handle_event(&page_down_event));
        assert_eq!(pane.scroll_offset, 10);

        // Test page up
        let page_up_event = AppEvent::Key(KeyEvent::from(KeyCode::PageUp));
        assert!(pane.handle_event(&page_up_event));
        assert_eq!(pane.scroll_offset, 0);

        // Test go to bottom (Shift+G)
        let bottom_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT));
        assert!(pane.handle_event(&bottom_event));
        assert_eq!(pane.scroll_offset, 19);

        // Test go to top (g)
        let top_event = AppEvent::Key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert!(pane.handle_event(&top_event));
        assert_eq!(pane.scroll_offset, 0);
    }

    #[test]
    fn test_commit_summary_pane_llm_summary() {
        let mut pane = CommitSummaryPane::new();

        // Test that initially there's no summary
        assert!(pane.llm_summary.is_none());
        assert!(!pane.is_loading_summary);

        // Test that we can manually set a summary (for testing purposes)
        pane.llm_summary = Some("This is a test summary".to_string());
        assert!(pane.llm_summary.is_some());
        assert_eq!(pane.llm_summary.as_ref().unwrap(), "This is a test summary");
    }

    #[test]
    fn test_commit_summary_pane_with_llm_client() {
        use crate::config::LlmConfig;

        // Create a test LLM client
        let mut llm_config = LlmConfig::default();
        llm_config.api_key = Some("test_key".to_string());
        let llm_client = LlmClient::new(llm_config).ok();

        let pane = CommitSummaryPane::new_with_llm_client(llm_client);

        // Test that the pane has an LLM client
        assert!(pane.llm_client.is_some());
        assert!(pane.llm_summary.is_none());
        assert!(!pane.is_loading_summary);

        // Test that a pane without LLM client works too
        let pane_no_llm = CommitSummaryPane::new_with_llm_client(None);
        assert!(pane_no_llm.llm_client.is_none());
        assert!(pane_no_llm.llm_summary.is_none());
        assert!(!pane_no_llm.is_loading_summary);
    }

    #[test]
    fn test_commit_files_display_immediately() {
        let mut pane = CommitSummaryPane::new();

        // Create a commit with file changes (simulating data from get_commit_history)
        let commit = crate::git::CommitInfo {
            sha: "abc123".to_string(),
            short_sha: "abc123".to_string(),
            message: "Test commit".to_string(),
            author: "Test Author".to_string(),
            date: "2023-01-01".to_string(),
            files_changed: vec![
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("src/main.rs"),
                    status: crate::git::FileChangeStatus::Modified,
                    additions: 10,
                    deletions: 5,
                },
                crate::git::CommitFileChange {
                    path: std::path::PathBuf::from("src/lib.rs"),
                    status: crate::git::FileChangeStatus::Added,
                    additions: 20,
                    deletions: 0,
                },
            ],
        };

        // Update the pane with the commit
        pane.update_commit(Some(commit.clone()));

        // Verify that the pane is immediately in Loaded state (not LoadingFiles)
        assert_eq!(pane.loading_state, CommitSummaryLoadingState::Loaded);

        // Verify that the commit data is available
        assert!(pane.current_commit.is_some());
        let current_commit = pane.current_commit.as_ref().unwrap();
        assert_eq!(current_commit.files_changed.len(), 2);
        assert_eq!(
            current_commit.files_changed[0].path,
            std::path::PathBuf::from("src/main.rs")
        );
        assert_eq!(
            current_commit.files_changed[1].path,
            std::path::PathBuf::from("src/lib.rs")
        );

        // LLM summary should still be None (not loaded yet)
        assert!(pane.llm_summary.is_none());

        // But the files should be immediately available for display
        // (This would be verified in the render method, which would show files immediately)
    }

    #[test]
    fn test_pane_visibility() {
        let registry = create_test_pane_registry();

        let file_tree = registry.get_pane(&PaneId::FileTree).unwrap();
        assert!(file_tree.visible());

        let monitor = registry.get_pane(&PaneId::Monitor).unwrap();
        assert!(!monitor.visible());

        let status_bar = registry.get_pane(&PaneId::StatusBar).unwrap();
        assert!(status_bar.visible());
    }

    #[test]
    fn test_pane_ids() {
        assert_eq!(PaneId::FileTree, PaneId::FileTree);
        assert_ne!(PaneId::FileTree, PaneId::Monitor);
    }

    #[test]
    fn test_advice_generation_api_contract() {
        // Test contract for advice generation API methods
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that generate_advice method exists and works
        let result = panel.generate_advice("sample diff content");
        assert!(result.is_ok(), "generate_advice should not panic and return Result");

        let improvements = result.unwrap();
        // Should return empty vector for now (placeholder implementation)
        assert!(improvements.is_empty(), "Initial implementation should return empty improvements");

        // Test async advice generation methods exist
        let async_result = panel.start_async_advice_generation("sample diff");
        assert!(async_result.is_ok(), "start_async_advice_generation should work");

        let status = panel.get_advice_generation_status();
        assert_eq!(status, "Ready", "Should report ready status");
    }

    #[test]
    fn test_advice_generation_with_empty_diff_contract() {
        // Test advice generation handles empty diff gracefully
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        let result = panel.generate_advice("");
        assert!(result.is_ok(), "Should handle empty diff without error");

        let improvements = result.unwrap();
        assert!(improvements.is_empty(), "Empty diff should result in no improvements");
    }

    #[test]
    fn test_advice_generation_error_handling_contract() {
        // Test advice generation error handling
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test with invalid diff content
        let invalid_diff = "This is not a valid git diff";
        let result = panel.generate_advice(invalid_diff);

        assert!(result.is_ok(), "Should handle invalid input gracefully");
        let improvements = result.unwrap();
        assert!(improvements.is_empty(), "Invalid input should result in no improvements");
    }

    #[test]
    fn test_chat_functionality_api_contract() {
        // Test contract for chat functionality API methods
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test that chat methods exist and work
        let send_result = panel.send_chat_message("Hello, AI!");
        assert!(send_result.is_ok(), "send_chat_message should work");

        // Test chat history management (after sending a message, should have both user and AI messages)
        let history = panel.get_chat_history();
        assert_eq!(history.len(), 2, "Chat history should contain user message and AI response");
        assert_eq!(history[0].role, MessageRole::User, "First message should be from user");
        assert_eq!(history[1].role, MessageRole::Assistant, "Second message should be from AI");

        // Test clearing chat history
        let clear_result = panel.clear_chat_history();
        assert!(clear_result.is_ok(), "clear_chat_history should work");

        // Test error handling methods
        let last_error = panel.get_last_chat_error();
        assert!(last_error.is_none(), "Initial last error should be None");

        let is_available = panel.is_chat_available();
        assert!(is_available, "Chat should be available by default");
    }

    #[test]
    fn test_send_chat_message_contract() {
        // Test sending chat messages functionality
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send a chat message
        let result = panel.send_chat_message("Can you explain this code change?");
        assert!(result.is_ok(), "Should send chat message without error");

        // The implementation should now store both user and AI messages
        let history = panel.get_chat_history();
        assert_eq!(history.len(), 2, "Should have user message and AI response");
        assert_eq!(history[0].role, MessageRole::User, "First message should be from user");
        assert_eq!(history[1].role, MessageRole::Assistant, "Second message should be from AI");
        assert!(history[0].content.contains("explain this code change"), "User message should be preserved");
    }

    #[test]
    fn test_chat_message_validation_contract() {
        // Test validation of chat messages
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test various message types
        let long_message = "a".repeat(10000);
        let test_messages = vec![
            "", // Empty message
            "   ", // Whitespace-only
            &long_message, // Very long message
            "Hello 🚀! Special chars: @#$%^&*()", // Unicode and special chars
        ];

        for message in test_messages {
            let result = panel.send_chat_message(message);
            let preview = message.chars().take(20).collect::<String>();
            assert!(result.is_ok(), "Should handle message: '{}'", preview);
        }
    }

    #[test]
    fn test_chat_history_management_contract() {
        // Test chat history management
        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Send multiple messages
        for i in 0..5 {
            let result = panel.send_chat_message(&format!("Message {}", i));
            assert!(result.is_ok(), "Should send message {}", i);
        }

        // Clear history
        let clear_result = panel.clear_chat_history();
        assert!(clear_result.is_ok(), "Should clear chat history");

        // History should be empty after clearing
        let history = panel.get_chat_history();
        assert!(history.is_empty(), "History should be empty after clearing");
    }

    #[test]
    fn test_panel_opening_integration_contract() {
        // Test integration contract for panel opening flow
        use crate::ui::App;
        use std::sync::Arc;

        // Create app using the same pattern as existing tests
        let mut llm_config = crate::config::LlmConfig::default();
        if std::env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = crate::llm::LlmClient::new(llm_config).ok();
        let llm_state = Arc::new(crate::shared_state::LlmSharedState::new());
        let mut app = App::new_with_config(true, true, crate::ui::Theme::Dark, llm_client, llm_state);

        // Test that toggle_pane_visibility works for advice panel
        // This is the core integration contract - Ctrl+L should work
        let result1 = app.toggle_pane_visibility(&PaneId::Advice);
        assert!(result1.is_ok(), "Should be able to toggle advice panel visibility");

        // Test toggling back
        let result2 = app.toggle_pane_visibility(&PaneId::Advice);
        assert!(result2.is_ok(), "Should be able to toggle advice panel back to hidden");

        // Test error handling for invalid pane ID
        let invalid_result = app.toggle_pane_visibility(&PaneId::FileTree); // FileTree is not togglable
        // This should either succeed or give a meaningful error
        assert!(invalid_result.is_ok(), "Should handle invalid pane gracefully");
    }

    #[test]
    fn test_panel_opening_keyboard_integration_contract() {
        // Test contract for keyboard integration - this represents what happens in main.rs
        // We can't directly test key events here, but we can test the method that gets called
        use crate::ui::App;
        use std::sync::Arc;

        // Create app with same pattern as tests
        let mut llm_config = crate::config::LlmConfig::default();
        if std::env::var("OPENAI_API_KEY").is_err() {
            llm_config.api_key = Some("dummy_key".to_string());
        }
        let llm_client = crate::llm::LlmClient::new(llm_config).ok();
        let llm_state = Arc::new(crate::shared_state::LlmSharedState::new());
        let mut app = App::new_with_config(true, true, crate::ui::Theme::Dark, llm_client, llm_state);

        // Test multiple toggles to simulate repeated Ctrl+L presses
        for i in 0..5 {
            let result = app.toggle_pane_visibility(&PaneId::Advice);
            assert!(result.is_ok(), "Ctrl+L toggle {} should succeed", i + 1);
        }
    }

    #[test]
    fn test_chat_conversation_flow_integration_contract() {
        // Test integration contract for complete chat conversation flow
        use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
        use crate::pane::AppEvent;

        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test initial state - should be in Viewing mode
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should start in Viewing mode");

        // Step 1: Enter chat mode with '/' key
        let slash_key = AppEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        let handled = panel.handle_event(&slash_key);
        assert!(handled, "Should handle '/' key to enter chat mode");
        assert_eq!(panel.get_mode(), AdviceMode::Chatting, "Should be in Chatting mode after '/'");

        // Step 2: Test typing in chat input
        let test_chars = vec![
            KeyEvent::new(KeyCode::Char('H'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE),
            KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
        ];

        for char_key in test_chars {
            let handled = panel.handle_event(&AppEvent::Key(char_key));
            assert!(handled, "Should handle character input in chat mode");
        }

        // Chat input should contain the typed text
        assert_eq!(panel.chat_input, "Hello", "Chat input should reflect typed characters");

        // Step 3: Test backspace functionality
        let backspace_key = AppEvent::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
        for _ in 0..2 {
            let handled = panel.handle_event(&backspace_key);
            assert!(handled, "Should handle backspace in chat mode");
        }

        assert_eq!(panel.chat_input, "Hel", "Backspace should remove characters");

        // Step 4: Test sending message with Enter key
        let enter_key = AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let handled = panel.handle_event(&enter_key);
        assert!(handled, "Should handle Enter key to send message");

        // Chat input should be cleared after sending
        assert_eq!(panel.chat_input, "", "Chat input should be cleared after sending");

        // Message should be in chat history (implementation dependent)
        let _history = panel.get_chat_history(); // Method should work
        assert!(panel.is_chat_available(), "Chat should remain available after sending message");

        // Step 5: Test exit chat mode with ESC
        let esc_key = AppEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let handled = panel.handle_event(&esc_key);
        assert!(handled, "Should handle ESC key to exit chat mode");
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should return to Viewing mode after ESC");

        // Step 6: Test re-entering chat mode
        let handled = panel.handle_event(&slash_key);
        assert!(handled, "Should be able to re-enter chat mode");
        assert_eq!(panel.get_mode(), AdviceMode::Chatting, "Should be back in Chatting mode");
    }

    #[test]
    fn test_chat_conversation_error_handling_integration_contract() {
        // Test error handling in chat conversation flow
        use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
        use crate::pane::AppEvent;

        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Enter chat mode
        let slash_key = AppEvent::Key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        let handled = panel.handle_event(&slash_key);
        assert!(handled, "Should enter chat mode successfully");

        // Test sending empty message
        let enter_key = AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        let handled = panel.handle_event(&enter_key);
        assert!(handled, "Should handle sending empty message gracefully");

        // Test typing very long message
        let long_text = "a".repeat(1000);
        for char in long_text.chars() {
            let key_event = AppEvent::Key(KeyEvent::new(KeyCode::Char(char), KeyModifiers::NONE));
            let _handled = panel.handle_event(&key_event); // Should handle gracefully
        }

        // Should still be functional after long input
        assert_eq!(panel.get_mode(), AdviceMode::Chatting, "Should remain in chat mode");
        assert!(panel.is_chat_available(), "Chat should still be available");
    }

    #[test]
    fn test_help_system_integration_contract() {
        // Test integration contract for help system
        use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
        use crate::pane::AppEvent;

        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Test initial state - should be in Viewing mode
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should start in Viewing mode");

        // Step 1: Enter help mode with '?' key
        let question_key = AppEvent::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let handled = panel.handle_event(&question_key);
        assert!(handled, "Should handle '?' key to enter help mode");
        assert_eq!(panel.get_mode(), AdviceMode::Help, "Should be in Help mode after '?'");

        // Step 2: Test navigation in help mode
        let nav_keys = vec![
            AppEvent::Key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE)), // Down
            AppEvent::Key(KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE)), // Up
        ];

        for nav_key in nav_keys {
            let handled = panel.handle_event(&nav_key);
            assert!(handled, "Should handle navigation keys in help mode");
            // Should remain in help mode during navigation
            assert_eq!(panel.get_mode(), AdviceMode::Help, "Should stay in Help mode during navigation");
        }

        // Step 3: Test exit help mode with ESC
        let esc_key = AppEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let handled = panel.handle_event(&esc_key);
        assert!(handled, "Should handle ESC key to exit help mode");
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should return to Viewing mode after ESC");

        // Step 4: Test re-entering help mode
        let handled = panel.handle_event(&question_key);
        assert!(handled, "Should be able to re-enter help mode");
        assert_eq!(panel.get_mode(), AdviceMode::Help, "Should be back in Help mode");
    }

    #[test]
    fn test_help_system_from_different_modes_integration_contract() {
        // Test help system accessibility from different modes
        use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
        use crate::pane::AppEvent;

        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        let _modes_to_test = vec![
            (AdviceMode::Viewing, "Viewing mode"),
            // Note: We can't directly set internal modes, but we can test transitions
        ];

        let question_key = AppEvent::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        let esc_key = AppEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

        // Test from viewing mode
        let handled = panel.handle_event(&question_key);
        assert!(handled, "Should enter help from viewing mode");
        assert_eq!(panel.get_mode(), AdviceMode::Help, "Should be in help mode");

        // Exit and test again
        let handled = panel.handle_event(&esc_key);
        assert!(handled, "Should exit help mode");
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should return to viewing mode");

        // Test entering help mode multiple times
        for i in 0..3 {
            let handled = panel.handle_event(&question_key);
            assert!(handled, "Should enter help mode on attempt {}", i + 1);
            assert_eq!(panel.get_mode(), AdviceMode::Help, "Should be in help mode");

            let handled = panel.handle_event(&esc_key);
            assert!(handled, "Should exit help mode on attempt {}", i + 1);
            assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should return to viewing mode");
        }
    }

    #[test]
    fn test_help_system_error_handling_integration_contract() {
        // Test error handling in help system
        use crossterm::event::{KeyEvent, KeyCode, KeyModifiers};
        use crate::pane::AppEvent;

        let config = Config::default();
        let advice_config = AdviceConfig::default();
        let mut panel = AdvicePanel::new(config, advice_config).unwrap();

        // Enter help mode
        let question_key = AppEvent::Key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE));
        panel.handle_event(&question_key);

        // Test various key inputs in help mode - should handle gracefully
        let test_keys = vec![
            AppEvent::Key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE)), // Regular char
            AppEvent::Key(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::NONE)), // Number
            AppEvent::Key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),  // Enter
            AppEvent::Key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),    // Tab
            AppEvent::Key(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)), // Backspace
        ];

        for test_key in test_keys {
            let _handled = panel.handle_event(&test_key);
            // Most keys should not be handled in help mode (only j, k, ESC)
            // but should not cause errors
            // The important thing is that the panel remains stable
            assert_eq!(panel.get_mode(), AdviceMode::Help, "Should remain in help mode after key input");
        }

        // Test that we can still exit normally
        let esc_key = AppEvent::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        let handled = panel.handle_event(&esc_key);
        assert!(handled, "Should be able to exit help mode after various key inputs");
        assert_eq!(panel.get_mode(), AdviceMode::Viewing, "Should return to viewing mode");
    }
}
