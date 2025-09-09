use log::debug;
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;
use tokio::sync::mpsc;

#[derive(Debug)]
pub struct AsyncMonitorCommand {
    result_rx: mpsc::Receiver<MonitorResult>,
    last_run: std::sync::Arc<std::sync::RwLock<Option<std::time::Instant>>>,
}

#[derive(Debug, Clone)]
pub enum MonitorResult {
    Success(String),
    Error(String),
}

impl AsyncMonitorCommand {
    pub fn new(command: String, interval: u64) -> Self {
        let (result_tx, result_rx) = mpsc::channel(32);
        let last_run = std::sync::Arc::new(std::sync::RwLock::new(None));

        let command_clone = command.clone();
        let last_run_clone = last_run.clone();
        tokio::spawn(async move {
            let mut last_run: Option<Instant> = None;

            loop {
                let should_run = if let Some(last_run_time) = last_run {
                    last_run_time.elapsed() >= Duration::from_secs(interval)
                } else {
                    true
                };

                if should_run {
                    debug!("Running async monitor command: {}", command_clone);

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

                    match result {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);

                            if output.status.success() {
                                let output_str = if stderr.is_empty() {
                                    format!("$ {}\n{}", command_clone, stdout)
                                } else {
                                    format!("$ {}\n{}\n{}", command_clone, stdout, stderr)
                                };

                                if result_tx
                                    .send(MonitorResult::Success(output_str))
                                    .await
                                    .is_err()
                                {
                                    break; // Channel closed, stop the task
                                }
                                debug!("Async monitor command completed successfully");
                            } else {
                                let error_str = format!(
                                    "$ {}\nCommand failed: {}\n{}",
                                    command_clone, stderr, stdout
                                );

                                if result_tx
                                    .send(MonitorResult::Error(error_str))
                                    .await
                                    .is_err()
                                {
                                    break; // Channel closed, stop the task
                                }
                                debug!("Async monitor command failed: {}", stderr);
                            }
                        }
                        Err(e) => {
                            let error_str =
                                format!("$ {}\nCommand execution failed: {}", command_clone, e);

                            if result_tx
                                .send(MonitorResult::Error(error_str))
                                .await
                                .is_err()
                            {
                                break; // Channel closed, stop the task
                            }
                            debug!("Async monitor command execution error: {}", e);
                        }
                    }

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
            result_rx,
            last_run,
        }
    }

    pub fn try_get_result(&mut self) -> Option<MonitorResult> {
        match self.result_rx.try_recv() {
            Ok(result) => Some(result),
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(_) => None,
        }
    }

    pub fn get_elapsed_since_last_run(&self) -> Option<Duration> {
        if let Ok(last_run) = self.last_run.read() {
            last_run.map(|instant| instant.elapsed())
        } else {
            None
        }
    }

    pub fn has_run_yet(&self) -> bool {
        if let Ok(last_run) = self.last_run.read() {
            last_run.is_some()
        } else {
            false
        }
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
