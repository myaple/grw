use std::cell::RefCell;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyModifiers};
use log::debug;
use ratatui::{
    Frame,
    layout::Rect,
    prelude::Stylize,
    style::Style,
    text::Line,
    widgets::{Block, Borders, Paragraph, Wrap},
};
use unicode_segmentation::UnicodeSegmentation;

use super::{AppEvent, Pane};
use crate::git::GitRepo;
use crate::llm::LlmClient;
use crate::shared_state::LlmSharedState;
use crate::ui::App;

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
}

#[derive(Debug, Clone, PartialEq)]
pub enum LoadingState {
    Idle,
    GeneratingAdvice,
    SendingChat,
}

#[derive(Debug)]
pub struct AdvicePanel {
    pub visible: bool,
    pub mode: AdviceMode,
    pub content: AdviceContent,
    pub chat_input: String,
    pub chat_input_active: bool,
    pub scroll_offset: usize,
    pub shared_state: Option<Arc<LlmSharedState>>,
    pub llm_client: Option<Arc<tokio::sync::Mutex<LlmClient>>>,
    pub current_diff_hash: Option<String>,
    pub loading_state: LoadingState,
    pub pending_advice_task: Option<tokio::task::JoinHandle<()>>,
    pub pending_chat_task: Option<tokio::task::JoinHandle<()>>,
    pub pending_chat_message_id: Option<String>,
    pub current_diff_content: RefCell<Option<String>>,
    pub max_tokens: usize, // Cache the max_tokens from config
    pub initial_message_sent: bool,
    pub first_visit: bool,
    pub chat_content_backup: Option<AdviceContent>,
    pub needs_initialization: bool,
}

impl AdvicePanel {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            visible: false,
            mode: AdviceMode::Chatting,
            content: AdviceContent::Loading,
            chat_input: String::new(),
            chat_input_active: false,
            scroll_offset: 0,
            shared_state: None,
            llm_client: None,
            current_diff_hash: None,
            loading_state: LoadingState::Idle,
            pending_advice_task: None,
            pending_chat_task: None,
            pending_chat_message_id: None,
            current_diff_content: RefCell::new(None),
            max_tokens: 16000, // Default value, will be updated when config is available
            initial_message_sent: false,
            first_visit: true,
            chat_content_backup: None,
            needs_initialization: false,
        })
    }

    /// Set the shared state for the advice panel
    pub fn set_shared_state(&mut self, shared_state: Arc<LlmSharedState>) {
        self.shared_state = Some(shared_state);
    }

    /// Set the LLM client for the advice panel
    pub fn set_llm_client(&mut self, llm_client: Arc<tokio::sync::Mutex<LlmClient>>) {
        debug!("🎯 ADVICE_PANEL: LLM client has been set");
        self.llm_client = Some(llm_client);
    }

    /// Set max_tokens directly (for testing or when config is available separately)
    pub fn set_max_tokens(&mut self, max_tokens: usize) {
        self.max_tokens = max_tokens;
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
        let conversation_history = self.get_chat_history();

        // Store the message ID for tracking the response
        self.pending_chat_message_id = Some(user_message_id.clone());

        // Clone necessary data for the async task
        let shared_state_clone = self.shared_state.clone();
        let llm_client_clone = self.llm_client.clone();
        let message_id_clone = user_message_id.clone();
        let message_content = message.to_string();

        // Spawn async task for chat response generation
        let task = tokio::spawn(async move {
            debug!(
                "🎯 ADVICE_PANEL: Async chat task started for message: {}",
                message_content
            );

            let result = async {
                // Try to use LLM client if available
                if let Some(llm_client) = llm_client_clone {
                    let client = llm_client.lock().await;
                    debug!("🎯 ADVICE_PANEL: About to call LLM send_chat_followup");

                    match client
                        .send_chat_followup(message_content, conversation_history)
                        .await
                    {
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
            }
            .await;

            // Store results in shared state
            if let Some(shared_state) = shared_state_clone {
                match result {
                    Ok(ai_message) => {
                        shared_state
                            .store_pending_chat_response(message_id_clone.clone(), ai_message);
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
        debug!(
            "🎯 ADVICE_PANEL: Spawned async chat generation task with message ID: {}",
            user_message_id
        );

        Ok(())
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
        if let (Some(shared_state), Some(diff_hash)) = (&self.shared_state, &self.current_diff_hash)
        {
            // Check if we have results for this diff
            if let Some(results) = shared_state.get_advice_results(diff_hash) {
                debug!(
                    "🎯 ADVICE_PANEL: Updating panel with {} improvements from shared state",
                    results.len()
                );
                self.content = AdviceContent::Improvements(results);
                self.update_advice_status(LoadingState::Idle);
            } else if let Some(error) =
                shared_state.get_advice_error(&format!("advice_{}", diff_hash))
            {
                debug!(
                    "🎯 ADVICE_PANEL: Updating panel with error from shared state: {}",
                    error
                );
                let improvements = vec![AdviceImprovement {
                    id: uuid::Uuid::new_v4().to_string(),
                    title: "Advice Generation Failed".to_string(),
                    description: format!("Failed to generate advice: {}", error),
                    priority: ImprovementPriority::High,
                    category: "Error".to_string(),
                    code_examples: Vec::new(),
                }];
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
                } else if let Some(error) =
                    shared_state.get_advice_error(&format!("chat_{}", message_id))
                {
                    debug!(
                        "🎯 ADVICE_PANEL: Updating chat with error from shared state: {}",
                        error
                    );

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

        // Apply truncation to diff content using config max_tokens
        // Convert tokens to characters using 3 chars per token ratio
        let max_chars = self.max_tokens * 3;
        let truncated_diff = if diff_content.len() > max_chars {
            let truncated = diff_content.chars().take(max_chars).collect::<String>();
            format!("{}\n\n[... diff truncated for brevity ...]", truncated)
        } else {
            diff_content.to_string()
        };

        let initial_message = format!(
            "Please provide 3 actionable improvements for the following code changes:\n\n```diff\n{}\n```\n\nFocus on practical, specific suggestions that would improve code quality, performance, or maintainability.",
            truncated_diff
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

    pub fn refresh_chat_with_new_diff(&mut self) {
        debug!("🎯 ADVICE_PANEL: Refreshing chat with new diff");

        // Clear existing chat content
        self.content = AdviceContent::Chat(Vec::new());
        self.scroll_offset = 0;

        // Reset first visit flag so it will send a new initial message
        self.first_visit = true;
        self.initial_message_sent = false;
        self.needs_initialization = true;

        // The new message will be sent on the next initialization cycle
    }

    /// Format chat content to preserve markdown, code blocks, and spacing
    fn format_chat_content(&self, content: &str, theme: &crate::ui::Theme) -> Vec<Line<'_>> {
        use ratatui::style::Style;
        use ratatui::text::Span;

        let mut lines = Vec::new();
        let mut in_code_block = false;
        let mut current_paragraph = String::new();

        for line in content.lines() {
            let trimmed = line.trim();

            // Handle code block markers
            if trimmed.starts_with("```") {
                if in_code_block {
                    // End of code block
                    if !current_paragraph.is_empty() {
                        // Add any remaining paragraph text before code block ended
                        for para_line in textwrap::wrap(&current_paragraph, 76) {
                            lines.push(Line::from(Span::styled(
                                format!("  {}", para_line),
                                Style::default().fg(theme.foreground_color()),
                            )));
                        }
                        current_paragraph.clear();
                    }
                    in_code_block = false;
                    lines.push(Line::from(Span::styled(
                        "  ```",
                        Style::default().fg(theme.secondary_color()),
                    )));
                } else {
                    // Start of code block - flush any current paragraph
                    if !current_paragraph.is_empty() {
                        for para_line in textwrap::wrap(&current_paragraph, 76) {
                            lines.push(Line::from(Span::styled(
                                format!("  {}", para_line),
                                Style::default().fg(theme.foreground_color()),
                            )));
                        }
                        current_paragraph.clear();
                    }
                    in_code_block = true;
                    lines.push(Line::from(Span::styled(
                        "  ```",
                        Style::default().fg(theme.secondary_color()),
                    )));
                }
                continue;
            }

            if in_code_block {
                // Inside code block - preserve exact formatting and use monospace-like color
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(theme.primary_color()),
                )));
            } else if trimmed.is_empty() {
                // Empty line - flush current paragraph and add empty line
                if !current_paragraph.is_empty() {
                    for para_line in textwrap::wrap(&current_paragraph, 76) {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", para_line),
                            Style::default().fg(theme.foreground_color()),
                        )));
                    }
                    current_paragraph.clear();
                }
                lines.push(Line::from(""));
            } else if line.starts_with("    ") || line.starts_with("\t") {
                // Preserve indentation for code-like lines that aren't in formal code blocks
                if !current_paragraph.is_empty() {
                    // Flush current paragraph first
                    for para_line in textwrap::wrap(&current_paragraph, 76) {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", para_line),
                            Style::default().fg(theme.foreground_color()),
                        )));
                    }
                    current_paragraph.clear();
                }
                // Add indented line with code-like formatting
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(theme.primary_color()),
                )));
            } else if line.starts_with("#") || line.starts_with("##") || line.starts_with("###") {
                // Handle markdown headers
                if !current_paragraph.is_empty() {
                    for para_line in textwrap::wrap(&current_paragraph, 76) {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", para_line),
                            Style::default().fg(theme.foreground_color()),
                        )));
                    }
                    current_paragraph.clear();
                }
                // Add header with special formatting
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default()
                        .fg(theme.secondary_color())
                        .add_modifier(ratatui::style::Modifier::BOLD),
                )));
            } else if line.starts_with("- ") || line.starts_with("* ") {
                // Handle bullet points
                if !current_paragraph.is_empty() {
                    for para_line in textwrap::wrap(&current_paragraph, 76) {
                        lines.push(Line::from(Span::styled(
                            format!("  {}", para_line),
                            Style::default().fg(theme.foreground_color()),
                        )));
                    }
                    current_paragraph.clear();
                }
                // Add bullet point
                lines.push(Line::from(Span::styled(
                    format!("  {}", line),
                    Style::default().fg(theme.foreground_color()),
                )));
            } else {
                // Regular text - accumulate in paragraph
                if current_paragraph.is_empty() {
                    current_paragraph.push_str(line);
                } else {
                    current_paragraph.push(' ');
                    current_paragraph.push_str(line);
                }
            }
        }

        // Flush any remaining paragraph
        if !current_paragraph.is_empty() {
            for para_line in textwrap::wrap(&current_paragraph, 76) {
                lines.push(Line::from(Span::styled(
                    format!("  {}", para_line),
                    Style::default().fg(theme.foreground_color()),
                )));
            }
        }

        lines
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
                    if old_status == LoadingState::GeneratingAdvice
                        && let Some(ref diff_hash) = self.current_diff_hash
                    {
                        shared_state.complete_advice_task(diff_hash);
                    }
                }
                _ => {}
            }
        }
    }

    /// Get chat history for UI display
    pub fn get_chat_history(&self) -> Vec<ChatMessageData> {
        match &self.content {
            AdviceContent::Chat(messages) => messages.clone(),
            _ => Vec::new(),
        }
    }

    /// Initialize the panel with current diff content when it becomes visible
    pub fn initialize_with_current_diff(&mut self, files: &[crate::git::FileDiff]) {
        if !self.needs_initialization {
            return;
        }

        // Build diff content from files
        let mut diff_content = String::new();
        if !files.is_empty() {
            for file_diff in files {
                // Add file header
                diff_content.push_str(&format!(
                    "diff --git a/{} b/{}\n",
                    file_diff.path.to_string_lossy(),
                    file_diff.path.to_string_lossy()
                ));

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
        }

        // Store the diff content
        *self.current_diff_content.borrow_mut() = if diff_content.trim().is_empty() {
            None
        } else {
            Some(diff_content.clone())
        };

        // Send initial message if we have content and haven't sent it yet
        if !self.initial_message_sent && self.first_visit {
            if !diff_content.trim().is_empty() {
                self.send_initial_message_with_diff(&diff_content);
            } else {
                self.send_no_changes_message();
            }
            self.initial_message_sent = true;
            self.first_visit = false;
        }

        self.needs_initialization = false;
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

        // Update the current diff content from the app's files (read-only operation)
        let files = app.get_files();
        if !files.is_empty() {
            let mut diff_content = String::new();

            for file_diff in files {
                // Add file header
                diff_content.push_str(&format!(
                    "diff --git a/{} b/{}\n",
                    file_diff.path.to_string_lossy(),
                    file_diff.path.to_string_lossy()
                ));

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

        let block = Block::default()
            .title(self.title())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_color()));

        let content = match &self.content {
            AdviceContent::Loading => {
                vec![Line::from("Loading advice...".to_string())]
            }
            AdviceContent::Improvements(improvements) => {
                let mut lines = Vec::new();
                for imp in improvements {
                    // Use chat-style timestamp format
                    let time_str = "now"; // Improvements are always current
                    lines.push(Line::from(format!("[{}] AI:", time_str)).fg(theme.primary_color()));

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
                    lines.push(Line::from(format!(
                        "  {} {} · {}",
                        priority_emoji, imp.category, imp.priority
                    )));
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
                        MessageRole::User => ("You", theme.primary_color()),
                        MessageRole::Assistant => ("AI", theme.secondary_color()),
                        MessageRole::System => ("System", theme.highlight_color()),
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

                    // Add message content with better formatting preservation
                    let formatted_lines = self.format_chat_content(&msg.content, &theme);
                    for formatted_line in formatted_lines {
                        lines.push(formatted_line);
                    }
                    lines.push(Line::from("")); // Empty line between messages
                }

                // Add "thinking" indicator if waiting for AI response
                if self.loading_state == LoadingState::SendingChat
                    && self.pending_chat_message_id.is_some()
                {
                    lines.push(Line::from("[now] AI:").fg(theme.secondary_color()));
                    lines.push(
                        Line::from("  🤔 Thinking...".to_string()).fg(theme.highlight_color()),
                    );
                    lines.push(Line::from(""));
                }

                lines
            }
            AdviceContent::Help(help_text) => help_text.lines().map(Line::from).collect(),
        };

        // Determine layout based on whether chat input is active
        let (content_area, input_area) =
            if self.mode == AdviceMode::Chatting && self.chat_input_active {
                // Calculate dynamic height for the input box
                let input_text = format!("> {}", self.chat_input);
                let available_width = area.width.saturating_sub(4); // 2 for borders, 2 for padding
                let wrapped_lines = textwrap::wrap(&input_text, available_width as usize).len();
                let input_height = (wrapped_lines as u16 + 2).min(10).max(3); // Min 3, Max 10 lines

                let content_height = area.height.saturating_sub(input_height);

                let content_rect = Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: content_height,
                };

                let input_rect = Rect {
                    x: area.x,
                    y: area.y + content_height,
                    width: area.width,
                    height: input_height,
                };

                (content_rect, Some(input_rect))
            } else {
                (area, None)
            };

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: false })
            .scroll((self.scroll_offset as u16, 0));

        f.render_widget(paragraph, content_area);

        // Show chat input only when activated and if an area for it has been calculated
        if let Some(input_area) = input_area {
            if self.mode == AdviceMode::Chatting && self.chat_input_active {
                let input_block = Block::default()
                    .title("Chat Input")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border_color()));

                let input_text = format!("> {}", self.chat_input);
                let input_paragraph = Paragraph::new(input_text.clone())
                    .block(input_block.clone())
                    .wrap(Wrap { trim: false });

                f.render_widget(input_paragraph, input_area);

                // Calculate cursor position for potentially wrapped text
                let available_width = input_area.width.saturating_sub(4); // 2 for borders, 2 for padding
                if available_width > 0 {
                    let wrapped_lines = textwrap::wrap(&input_text, available_width as usize);
                    let mut cursor_x = input_area.x + 1;
                    let mut cursor_y = input_area.y + 1;

                    if let Some(last_line) = wrapped_lines.last() {
                        let last_line_graphemes = last_line.graphemes(true).count();
                        if last_line_graphemes < available_width as usize {
                            cursor_x += last_line_graphemes as u16;
                            cursor_y += (wrapped_lines.len() - 1) as u16;
                        } else {
                            // The last line is full, so the cursor should be at the start of the next line
                            cursor_y += wrapped_lines.len() as u16;
                        }
                    }

                    // Ensure cursor stays within the input area bounds
                    if cursor_y < input_area.bottom() - 1 {
                        f.set_cursor(cursor_x, cursor_y);
                    } else {
                        // If the cursor would be outside, place it at the last possible position
                        f.set_cursor(input_area.right() - 2, input_area.bottom() - 2);
                    }
                }
            }
        }

        Ok(())
    }

    fn handle_event(&mut self, event: &AppEvent) -> bool {
        match event {
            AppEvent::Key(key_event) => {
                // Handle Ctrl+R refresh separately (needs access to self method)
                if key_event.code == KeyCode::Char('r')
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                {
                    // Ctrl+R: Refresh diff and clear chat
                    self.refresh_chat_with_new_diff();
                    return true;
                }

                // Use the keys module for all other advice panel key handling
                super::keys::AdvicePanelKeyHandler::handle_advice_panel_keys(self, key_event)
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
            self.needs_initialization = true;
        }
    }

    fn as_advice_pane(&self) -> Option<&AdvicePanel> {
        Some(self)
    }

    fn as_advice_pane_mut(&mut self) -> Option<&mut AdvicePanel> {
        Some(self)
    }
}
