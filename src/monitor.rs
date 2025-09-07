use log::debug;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MonitorCommand {
    command: String,
    interval: u64,
    last_run: Option<Instant>,
    last_output: String,
}

impl MonitorCommand {
    pub fn new(command: String, interval: u64) -> Self {
        Self {
            command,
            interval,
            last_run: None,
            last_output: String::new(),
        }
    }

    pub fn should_run(&self) -> bool {
        if let Some(last_run) = self.last_run {
            last_run.elapsed() >= Duration::from_secs(self.interval)
        } else {
            true
        }
    }

    pub fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Running monitor command: {}", self.command);

        let output = if cfg!(target_os = "windows") {
            Command::new("cmd").args(["/C", &self.command]).output()?
        } else {
            Command::new("sh").args(["-c", &self.command]).output()?
        };

        self.last_run = Some(Instant::now());

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            self.last_output = if stderr.is_empty() {
                format!("$ {}\n{}", self.command, stdout)
            } else {
                format!("$ {}\n{}\n{}", self.command, stdout, stderr)
            };
            debug!("Monitor command completed successfully");
        } else {
            self.last_output =
                format!("$ {}\nCommand failed: {}\n{}", self.command, stderr, stdout);
            debug!("Monitor command failed: {}", stderr);
        }

        Ok(())
    }

    pub fn get_output(&self) -> &str {
        &self.last_output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_command_creation() {
        let monitor = MonitorCommand::new("echo test".to_string(), 5);
        assert_eq!(monitor.command, "echo test");
        assert_eq!(monitor.interval, 5);
        assert!(monitor.should_run());
        assert_eq!(monitor.get_output(), "");
    }

    #[test]
    fn test_monitor_command_should_run() {
        let mut monitor = MonitorCommand::new("echo test".to_string(), 1);

        // Initially should run
        assert!(monitor.should_run());

        // After setting last_run, should not run immediately
        monitor.last_run = Some(Instant::now());
        assert!(!monitor.should_run());
    }

    #[test]
    fn test_monitor_command_update_output() {
        let mut monitor = MonitorCommand::new("echo test".to_string(), 1);

        // Initially empty output
        assert_eq!(monitor.get_output(), "");

        // Update output
        monitor.last_output = "test output".to_string();
        assert_eq!(monitor.get_output(), "test output");
    }
}
