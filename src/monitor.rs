use log::debug;
use std::time::{Duration, Instant};
use tokio::process::Command as AsyncCommand;
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct MonitorOutput {
    pub output: String,
    pub timestamp: Instant,
}

#[derive(Debug)]
pub struct AsyncMonitorCommand {
    last_run: std::sync::Arc<std::sync::RwLock<Option<Instant>>>,
}

impl AsyncMonitorCommand {
    pub fn new(command: String, interval: u64) -> (Self, mpsc::Receiver<MonitorOutput>) {
        let (output_tx, output_rx) = mpsc::channel(32);
        let last_run = std::sync::Arc::new(std::sync::RwLock::new(None));

        let command_clone = command.clone();
        let tx_clone = output_tx.clone();
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
                    debug!("Running async monitor command: {command_clone}");
                    let _start_time = Instant::now();

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

                    let monitor_output = match result {
                        Ok(output) => {
                            let stdout = String::from_utf8_lossy(&output.stdout);
                            let stderr = String::from_utf8_lossy(&output.stderr);

                            if output.status.success() {
                                let output_str = if stderr.is_empty() {
                                    format!("$ {command_clone}\n{stdout}")
                                } else {
                                    format!("$ {command_clone}\n{stdout}\n{stderr}")
                                };

                                debug!("Async monitor command completed successfully");
                                MonitorOutput {
                                    output: output_str,
                                    timestamp: Instant::now(),
                                }
                            } else {
                                let error_str = format!(
                                    "$ {command_clone}\nCommand failed: {stderr}\n{stdout}"
                                );
                                debug!("Async monitor command failed: {stderr}");
                                MonitorOutput {
                                    output: error_str,
                                    timestamp: Instant::now(),
                                }
                            }
                        }
                        Err(e) => {
                            let error_str =
                                format!("$ {command_clone}\nCommand execution failed: {e}");
                            debug!("Async monitor command execution error: {e}");
                            MonitorOutput {
                                output: error_str,
                                timestamp: Instant::now(),
                            }
                        }
                    };

                    // Send output through channel
                    if let Err(e) = tx_clone.send(monitor_output.clone()).await {
                        debug!("Failed to send monitor output: {e}");
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

        (Self { last_run }, output_rx)
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
    use std::time::Duration;

    #[tokio::test]
    async fn test_monitor_command_creation() {
        let (monitor, mut rx) = AsyncMonitorCommand::new("echo test".to_string(), 1);

        assert!(!monitor.has_run_yet());
        assert!(monitor.get_elapsed_since_last_run().is_none());

        // Test that receiver works
        let initial_result = rx.try_recv();
        assert!(initial_result.is_err()); // Should be empty initially
    }

    #[tokio::test]
    async fn test_monitor_command_execution() {
        let (monitor, mut rx) = AsyncMonitorCommand::new("echo hello world".to_string(), 1);

        // Wait for the command to execute
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check that we received output
        let output = rx.try_recv();
        assert!(output.is_ok());

        let monitor_output = output.unwrap();
        assert!(monitor_output.output.contains("hello world"));
        assert!(monitor_output.output.contains("echo hello world"));

        // Check that monitor state was updated
        assert!(monitor.has_run_yet());
        assert!(monitor.get_elapsed_since_last_run().is_some());
    }

    #[tokio::test]
    async fn test_monitor_command_error_handling() {
        let (_monitor, mut rx) =
            AsyncMonitorCommand::new("nonexistent_command_12345".to_string(), 1);

        // Wait for the command to fail
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Check that we received error output
        let output = rx.try_recv();
        assert!(output.is_ok());

        let monitor_output = output.unwrap();
        assert!(monitor_output.output.contains("nonexistent_command_12345"));
    }

    #[tokio::test]
    async fn test_monitor_command_interval_respect() {
        let (_monitor, mut rx) = AsyncMonitorCommand::new("echo interval_test".to_string(), 2);

        // First execution should happen immediately
        tokio::time::sleep(Duration::from_millis(100)).await;
        let first_output = rx.try_recv();
        assert!(first_output.is_ok());

        // Clear the first output
        let _ = first_output;

        // Should not receive another output within 2 seconds
        tokio::time::sleep(Duration::from_secs(1)).await;
        let second_output = rx.try_recv();
        assert!(second_output.is_err());

        // Should receive another output after 2 seconds
        tokio::time::sleep(Duration::from_secs(2)).await;
        let third_output = rx.try_recv();
        assert!(third_output.is_ok());
    }

    #[tokio::test]
    async fn test_monitor_timing_methods() {
        let (monitor, _) = AsyncMonitorCommand::new("echo timing_test".to_string(), 1);

        // Initially should not have run
        assert!(!monitor.has_run_yet());
        assert!(monitor.get_elapsed_since_last_run().is_none());

        // Wait for execution
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should have run now
        assert!(monitor.has_run_yet());
        assert!(monitor.get_elapsed_since_last_run().is_some());

        let elapsed = monitor.get_elapsed_since_last_run().unwrap();
        assert!(elapsed.as_millis() > 0);
    }

    #[tokio::test]
    async fn test_monitor_output_structure() {
        let (_monitor, mut rx) = AsyncMonitorCommand::new("echo structured_output".to_string(), 1);

        // Wait for execution
        tokio::time::sleep(Duration::from_secs(2)).await;

        let output = rx.try_recv().unwrap();

        // Verify the output structure
        assert!(output.output.contains("echo structured_output"));
        assert!(output.output.starts_with("$ echo structured_output"));
        assert!(!output.output.is_empty());
    }

    #[tokio::test]
    async fn test_monitor_command_with_stderr() {
        let (_monitor, mut rx) =
            AsyncMonitorCommand::new("sh -c 'echo stdout; echo stderr >&2'".to_string(), 1);

        // Wait for execution
        tokio::time::sleep(Duration::from_secs(2)).await;

        let output = rx.try_recv().unwrap();

        // Should contain both stdout and stderr
        assert!(output.output.contains("stdout"));
        assert!(output.output.contains("stderr"));
    }

    #[tokio::test]
    async fn test_monitor_multiple_outputs() {
        let (_monitor, mut rx) = AsyncMonitorCommand::new("echo test".to_string(), 1);

        // Wait for first execution
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should have first output
        let first_output = rx.try_recv();
        assert!(first_output.is_ok());

        // Clear the first output
        let _ = first_output;

        // Wait for second execution
        tokio::time::sleep(Duration::from_secs(2)).await;

        // Should have second output
        let second_output = rx.try_recv();
        assert!(second_output.is_ok());

        // Verify outputs are different (different timestamps)
        let first = first_output.unwrap();
        let second = second_output.unwrap();
        assert!(first.timestamp != second.timestamp);
    }

    #[tokio::test]
    async fn test_monitor_channel_buffer() {
        // Test that channel buffers multiple outputs
        let (_monitor, mut rx) = AsyncMonitorCommand::new("echo buffer_test".to_string(), 1);

        // Wait for multiple executions
        tokio::time::sleep(Duration::from_secs(3)).await;

        // Should have multiple outputs in the buffer
        let mut count = 0;
        while rx.try_recv().is_ok() {
            count += 1;
            if count > 10 {
                break; // Safety check
            }
        }

        assert!(count >= 2); // Should have at least 2 outputs
    }
}
