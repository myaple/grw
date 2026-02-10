use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::debug;

use super::advice_panel::AdviceMode;
use crate::ui::App;

/// Key handling result type
pub enum KeyResult {
    /// Key was handled, don't process further
    Handled,
    /// Key was not handled, continue processing
    NotHandled,
    /// Key was handled and application should quit
    Quit,
}

// KeyResult implementation (to_bool method removed as unused)

/// Global application key handler
pub struct GlobalKeyHandler;

impl GlobalKeyHandler {
    /// Handle global application key events
    pub fn handle_global_key(app: &mut App, key: &KeyEvent) -> KeyResult {
        // Handle commit picker mode key events first
        if app.is_in_commit_picker_mode() {
            return Self::handle_commit_picker_keys(app, key);
        }

        // Let panes handle the key first
        let panes_handled = app.forward_key_to_panes(*key);
        if panes_handled {
            return KeyResult::Handled;
        }

        // Handle remaining global keys
        Self::handle_main_mode_keys(app, key)
    }

    /// Handle keys when in commit picker mode
    fn handle_commit_picker_keys(app: &mut App, key: &KeyEvent) -> KeyResult {
        match key.code {
            KeyCode::Char('q') => {
                log::info!("User requested quit from commit picker mode");
                KeyResult::Quit
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                log::info!("User requested quit via Ctrl+C from commit picker mode");
                KeyResult::Quit
            }
            KeyCode::Char('?') => {
                debug!("User pressed '?' in commit picker mode, toggling help");
                app.toggle_help();
                KeyResult::Handled
            }
            KeyCode::Esc => {
                debug!("User pressed Escape in commit picker mode, exiting");
                app.exit_commit_picker_mode();
                KeyResult::Handled
            }
            _ => {
                // Forward key events to commit picker pane with error handling
                let picker_handled = app.forward_key_to_commit_picker(*key);

                // Also forward to commit summary pane for scrolling if not handled by picker
                if !picker_handled {
                    app.forward_key_to_commit_summary(*key);
                }

                KeyResult::Handled // Don't quit, stay in commit picker mode
            }
        }
    }

    /// Handle keys when in main mode (not commit picker)
    fn handle_main_mode_keys(app: &mut App, key: &KeyEvent) -> KeyResult {
        match key.code {
            KeyCode::Char('q') => {
                log::info!("User requested quit");
                KeyResult::Quit
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                log::info!("User requested quit via Ctrl+C");
                KeyResult::Quit
            }
            KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_to_bottom(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::Char('j') if key.modifiers.is_empty() => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_down(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::Down => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_down(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::Char('k') if key.modifiers.is_empty() => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_up();
                KeyResult::Handled
            }
            KeyCode::Up => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_up();
                KeyResult::Handled
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_down(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::Char('y') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // This should have been handled by forward_key_to_panes above
                app.scroll_up();
                KeyResult::Handled
            }
            KeyCode::Char('g') => {
                if app.handle_g_press() {
                    KeyResult::Handled
                } else {
                    // g was pressed, wait for next key
                    KeyResult::Handled
                }
            }
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                debug!("User pressed Ctrl+T - toggling theme");
                app.toggle_theme();
                KeyResult::Handled
            }
            KeyCode::Char('t') => {
                // Check if g was pressed recently
                if let Some(last_time) = app.last_g_press
                    && std::time::Instant::now()
                        .duration_since(last_time)
                        .as_millis()
                        < 500
                {
                    debug!("User triggered 'gt' key combination - next file");
                    app.next_file();
                }
                KeyResult::Handled
            }
            KeyCode::Char('T') => {
                // Check if g was pressed recently
                if let Some(last_time) = app.last_g_press
                    && std::time::Instant::now()
                        .duration_since(last_time)
                        .as_millis()
                        < 500
                {
                    debug!("User triggered 'gT' key combination - previous file");
                    app.prev_file();
                }
                KeyResult::Handled
            }
            KeyCode::PageDown => {
                app.page_down(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::PageUp => {
                app.page_up(app.current_diff_height);
                KeyResult::Handled
            }
            KeyCode::Left => {
                debug!("User pressed Left - previous file");
                app.prev_file();
                KeyResult::Handled
            }
            KeyCode::Right => {
                debug!("User pressed Right - next file");
                app.next_file();
                KeyResult::Handled
            }
            KeyCode::Tab => {
                debug!("User pressed Tab - next file");
                app.next_file();
                KeyResult::Handled
            }
            KeyCode::BackTab => {
                debug!("User pressed Shift+Tab - previous file");
                app.prev_file();
                KeyResult::Handled
            }
            KeyCode::Char('?') => {
                app.toggle_help();
                KeyResult::Handled
            }
            KeyCode::Esc => {
                if app.is_showing_help() {
                    app.toggle_help();
                } else if app.is_advice_panel_visible() {
                    debug!("User pressed Escape - hiding advice panel and showing diff pane");
                    // Hide advice panel and show diff pane (same behavior as Ctrl+D)
                    if let Err(e) = app.toggle_pane_visibility(&super::PaneId::Advice) {
                        log::warn!("Failed to hide advice panel: {}", e);
                    }
                    if !app.is_diff_panel_visible()
                        && let Err(e) = app.toggle_pane_visibility(&super::PaneId::Diff)
                    {
                        log::warn!("Failed to show diff pane: {}", e);
                    }
                }
                KeyResult::Handled
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.set_side_by_side_diff();
                KeyResult::Handled
            }
            KeyCode::Char('h') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.toggle_diff_panel();
                KeyResult::Handled
            }
            KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.toggle_changed_files_pane();
                KeyResult::Handled
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.scroll_monitor_down();
                KeyResult::Handled
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::ALT) => {
                app.scroll_monitor_up();
                KeyResult::Handled
            }
            KeyCode::Char('m') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.toggle_monitor_pane();
                KeyResult::Handled
            }
            KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                debug!("User pressed Ctrl+L - toggling advice panel");
                if let Err(e) = app.toggle_pane_visibility(&super::PaneId::Advice) {
                    log::warn!("Failed to toggle advice panel: {}", e);
                }
                KeyResult::Handled
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                debug!("User pressed Ctrl+D - switching to diff pane");
                // Only handle Ctrl+D for advice panel navigation when advice panel is visible
                if app.is_advice_panel_visible() {
                    // Hide advice panel and show diff pane
                    if let Err(e) = app.toggle_pane_visibility(&super::PaneId::Advice) {
                        log::warn!("Failed to hide advice panel: {}", e);
                    }
                    if !app.is_diff_panel_visible()
                        && let Err(e) = app.toggle_pane_visibility(&super::PaneId::Diff)
                    {
                        log::warn!("Failed to show diff pane: {}", e);
                    }
                    KeyResult::Handled
                } else {
                    // Fall through to default Ctrl+D behavior (single pane diff)
                    app.set_single_pane_diff();
                    KeyResult::Handled
                }
            }
            _ => KeyResult::NotHandled,
        }
    }
}

/// Key handling utilities for panes
pub struct PaneKeyUtils;

impl PaneKeyUtils {
    /// Handle scrolling keys for any pane
    pub fn handle_scroll_keys(
        scroll_offset: &mut usize,
        key: &KeyEvent,
        content_line_count: usize,
    ) -> bool {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                *scroll_offset = scroll_offset.saturating_add(1);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                *scroll_offset = scroll_offset.saturating_sub(1);
                true
            }
            KeyCode::PageDown => {
                *scroll_offset = scroll_offset.saturating_add(10);
                true
            }
            KeyCode::PageUp => {
                *scroll_offset = scroll_offset.saturating_sub(10);
                true
            }
            KeyCode::Char('g') => {
                *scroll_offset = 0;
                true
            }
            KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                let visible_lines = 20; // Approximate visible area height
                *scroll_offset = content_line_count.saturating_sub(visible_lines).max(0);
                true
            }
            _ => false,
        }
    }
}

/// Advice panel specific key handling
pub struct AdvicePanelKeyHandler;

impl AdvicePanelKeyHandler {
    /// Handle key events specific to the advice panel
    pub fn handle_advice_panel_keys(
        advice_panel: &mut super::advice_panel::AdvicePanel,
        key: &KeyEvent,
    ) -> bool {
        use super::advice_panel::AdviceMode;

        match advice_panel.mode {
            AdviceMode::Chatting => Self::handle_chatting_mode_keys(advice_panel, key),
            AdviceMode::Help => Self::handle_help_mode_keys(advice_panel, key),
        }
    }

    /// Handle keys when in chat mode
    fn handle_chatting_mode_keys(
        advice_panel: &mut super::advice_panel::AdvicePanel,
        key: &KeyEvent,
    ) -> bool {
        if advice_panel.chat_input_active {
            // Chat input is active, handle input keys
            match key.code {
                KeyCode::Enter => {
                    if !advice_panel.chat_input.is_empty() {
                        let message = advice_panel.chat_input.clone();
                        advice_panel.chat_input.clear();
                        advice_panel.chat_input_active = false;
                        // Send the message
                        let _ = advice_panel.send_chat_message(&message);
                    }
                    true
                }
                KeyCode::Esc => {
                    advice_panel.chat_input_active = false;
                    advice_panel.chat_input.clear();
                    true
                }
                KeyCode::Backspace => {
                    advice_panel.chat_input.pop();
                    true
                }
                KeyCode::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+W: Delete last word
                    let chars: Vec<char> = advice_panel.chat_input.chars().collect();
                    let mut delete_pos = chars.len();
                    // Find the start of the last word by looking backwards
                    while delete_pos > 0 && chars[delete_pos - 1].is_whitespace() {
                        delete_pos -= 1;
                    }
                    while delete_pos > 0 && !chars[delete_pos - 1].is_whitespace() {
                        delete_pos -= 1;
                    }
                    // Rebuild the string up to delete_pos
                    advice_panel.chat_input = chars[..delete_pos].iter().collect();
                    true
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl+U: Delete entire line
                    advice_panel.chat_input.clear();
                    true
                }
                KeyCode::Char(c) => {
                    advice_panel.chat_input.push(c);
                    true
                }
                _ => false,
            }
        } else {
            // Chat input is not active, handle navigation and activation keys
            match key.code {
                KeyCode::Char('/') => {
                    advice_panel.chat_input_active = true;
                    true
                }
                KeyCode::Char('?') => {
                    advice_panel.mode = AdviceMode::Help;
                    // Reset scroll offset when entering help mode
                    advice_panel.scroll_offset = 0;
                    // Backup current chat content before switching to help
                    advice_panel.chat_content_backup = Some(advice_panel.content.clone());
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
                        "Panel Controls:",
                        "  Ctrl+L             - Toggle advice panel",
                        "  Ctrl+D             - Return to diff pane",
                        "  Ctrl+R             - Refresh diff and clear chat",
                        "  Esc                - Return to diff pane",
                        "  ?                  - Show this help",
                        "",
                        "Tips:",
                        "- Chat history is preserved across panel activations",
                        "- Initial message with diff is sent automatically on first visit",
                        "- Use Ctrl+R to refresh with latest diff and start fresh conversation",
                    ]
                    .join("\n");
                    advice_panel.content = super::advice_panel::AdviceContent::Help(help_text);
                    true
                }
                KeyCode::Esc => {
                    if advice_panel.chat_input_active {
                        // Deactivate chat input
                        advice_panel.chat_input_active = false;
                        advice_panel.chat_input.clear();
                        true
                    } else {
                        false // Let parent handle Esc for panel closing
                    }
                }
                // Handle scrolling
                key_code => {
                    let content_lines = match &advice_panel.content {
                        super::advice_panel::AdviceContent::Chat(messages) => {
                            let mut line_count = 0;
                            for msg in messages {
                                // Skip user messages that contain the diff pattern
                                if msg.role == super::advice_panel::MessageRole::User
                                    && msg.content.contains("Please provide 3 actionable improvements for the following code changes:") {
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
                            if advice_panel.loading_state
                                == super::advice_panel::LoadingState::SendingChat
                                && advice_panel.pending_chat_message_id.is_some()
                            {
                                line_count += 3; // "AI:" + "Thinking..." + empty line
                            }
                            line_count
                        }
                        super::advice_panel::AdviceContent::Help(help_text) => {
                            help_text.lines().count()
                        }
                        _ => 0,
                    };
                    let fake_key_event = KeyEvent::new(key_code, KeyModifiers::NONE);
                    PaneKeyUtils::handle_scroll_keys(
                        &mut advice_panel.scroll_offset,
                        &fake_key_event,
                        content_lines,
                    )
                }
            }
        }
    }

    /// Handle keys when in help mode
    fn handle_help_mode_keys(
        advice_panel: &mut super::advice_panel::AdvicePanel,
        key: &KeyEvent,
    ) -> bool {
        match key.code {
            KeyCode::Esc => {
                // Exit help mode and restore chat content
                advice_panel.mode = super::advice_panel::AdviceMode::Chatting;
                // Restore backed up chat content if available
                if let Some(backup_content) = advice_panel.chat_content_backup.take() {
                    advice_panel.content = backup_content;
                } else {
                    // Fallback to empty chat if no backup
                    advice_panel.content = super::advice_panel::AdviceContent::Chat(Vec::new());
                }
                false // Let parent handle Esc for panel closing
            }
            key_code => {
                let content_lines = match &advice_panel.content {
                    super::advice_panel::AdviceContent::Help(help_text) => {
                        help_text.lines().count()
                    }
                    _ => 0,
                };
                let fake_key_event = KeyEvent::new(key_code, KeyModifiers::NONE);
                PaneKeyUtils::handle_scroll_keys(
                    &mut advice_panel.scroll_offset,
                    &fake_key_event,
                    content_lines,
                )
            }
        }
    }
}
