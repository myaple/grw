use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use log::debug;

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