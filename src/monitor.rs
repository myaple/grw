use log::debug;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;

use crate::shared_state::{MonitorSharedState, MonitorTiming};

#[derive(Debug)]
pub struct AsyncMonitorCommand {
    shared_state: Arc<MonitorSharedState>,
    command_key: String,
    last_run: std::sync::Arc<std::sync::RwLock<Option<std::time::Instant>>>,
}

#[derive(Debug, Clone)]
pub enum MonitorResult {
    Success(String),
    Error(String),
}

impl AsyncMonitorCommand {
    pub fn new(command: String, interval: u64, shared_state: Arc<MonitorSharedState>) -> Self {
        let last_run = std::sync::Arc::new(std::sync::RwLock::new(None));
        let command_key = format!("monitor_command_{}", command);

        let command_clone = command.clone();
        let command_key_clone = command_key.clone();
        let last_run_clone = last_run.clone();
        let shared_state_clone = shared_state.clone();
        
        tokio::spawn(async move {
            let mut last_run: Option<Instant> = None;

            loop {
                let should_run = if let Some(last_run_time) = last_run {
                    last_run_time.elapsed() >= Duration::from_secs(interval)
                } else {
                    true
                };

                if should_run {
                    debug!("Running async monitor command: {command_clone}");
                    let start_time = Instant::now();

                    let result = if cfg!(target_os = "windows") {
                        AsyncCommand::new("cmd")
                            .args(["/C", &command_clone])
                            .output()
                            .await
                    } else {
                        AsyncCommand::new("sh")
                            .args(["-c", &command_clone])
                            .output()
                            .await
                    };

                    let elapsed = start_time.elapsed();

                    match result {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);

                            if output.status.success() {
                                let output_str = if stderr.is_empty() {
                                    format!("$ {command_clone}\n{stdout}")
                                } else {
                                    format!("$ {command_clone}\n{stdout}\n{stderr}")
                                };

                                // Store successful output in shared state
                                shared_state_clone.update_output(command_key_clone.clone(), output_str);
                                shared_state_clone.clear_error(&command_key_clone);
                                debug!("Async monitor command completed successfully");
                            } else {
                                let error_str = format!(
                                    "$ {command_clone}\nCommand failed: {stderr}\n{stdout}"
                                );

                                // Store error output in shared state
                                shared_state_clone.update_output(command_key_clone.clone(), error_str.clone());
                                shared_state_clone.set_error(command_key_clone.clone(), error_str);
                                debug!("Async monitor command failed: {stderr}");
                            }
                        }
                        Err(e) => {
                            let error_str =
                                format!("$ {command_clone}\nCommand execution failed: {e}");

                            // Store execution error in shared state
                            shared_state_clone.update_output(command_key_clone.clone(), error_str.clone());
                            shared_state_clone.set_error(command_key_clone.clone(), error_str);
                            debug!("Async monitor command execution error: {e}");
                        }
                    }

                    // Update timing information in shared state
                    let timing = MonitorTiming::with_current_time(elapsed.as_millis() as u64);
                    shared_state_clone.update_timing(command_key_clone.clone(), timing);

                    let now = Instant::now();
                    last_run = Some(now);
                    if let Ok(mut shared_last_run) = last_run_clone.write() {
                        *shared_last_run = Some(now);
                    }
                }

                // Sleep for a short duration before checking again
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        });

        Self {
            shared_state,
            command_key,
            last_run,
        }
    }

    pub fn try_get_result(&mut self) -> Option<MonitorResult> {
        // Check if there's new output in shared state
        if let Some(output) = self.shared_state.get_output(&self.command_key) {
            // Check if there's an error for this command
            if let Some(_error) = self.shared_state.get_error(&self.command_key) {
                Some(MonitorResult::Error(output))
            } else {
                Some(MonitorResult::Success(output))
            }
        } else {
            None
        }
    }

    pub fn get_elapsed_since_last_run(&self) -> Option<Duration> {
        // First try to get from shared state timing
        if let Some(timing) = self.shared_state.get_timing(&self.command_key) {
            if timing.has_run {
                let current_time = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let elapsed_since_last = current_time.saturating_sub(timing.last_run);
                return Some(Duration::from_secs(elapsed_since_last));
            }
        }
        
        // Fallback to local timing for backward compatibility
        if let Ok(last_run) = self.last_run.read() {
            last_run.map(|instant| instant.elapsed())
        } else {
            None
        }
    }

    pub fn has_run_yet(&self) -> bool {
        // Check shared state first
        if let Some(timing) = self.shared_state.get_timing(&self.command_key) {
            return timing.has_run;
        }
        
        // Fallback to local state
        if let Ok(last_run) = self.last_run.read() {
            last_run.is_some()
        } else {
            false
        }
    }

    /// Get the command key used for shared state storage
    pub fn get_command_key(&self) -> &str {
        &self.command_key
    }

    /// Get direct access to shared state for advanced operations
    pub fn get_shared_state(&self) -> &Arc<MonitorSharedState> {
        &self.shared_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_result_types() {
        let success = MonitorResult::Success("test output".to_string());
        let error = MonitorResult::Error("error output".to_string());

        match success {
            MonitorResult::Success(output) => assert_eq!(output, "test output"),
            MonitorResult::Error(_) => panic!("Expected success"),
        }

        match error {
            MonitorResult::Success(_) => panic!("Expected error"),
            MonitorResult::Error(output) => assert_eq!(output, "error output"),
        }
    }
}
