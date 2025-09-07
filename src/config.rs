use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub debug: bool,
    pub no_diff: bool,
    pub monitor_command: Option<String>,
    pub monitor_interval: Option<u64>,
}

impl Config {
    pub fn load() -> color_eyre::eyre::Result<Self> {
        let config_path = Self::get_config_path();

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path)?;
        let config: Config = serde_json::from_str(&content)?;
        Ok(config)
    }

    fn get_config_path() -> PathBuf {
        config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("grw")
            .join("config.json")
    }

    pub fn merge_with_args(&self, args: &Args) -> Self {
        Self {
            debug: args.debug || self.debug,
            no_diff: args.no_diff || self.no_diff,
            monitor_command: args
                .monitor_command
                .clone()
                .or_else(|| self.monitor_command.clone()),
            monitor_interval: args.monitor_interval.or(self.monitor_interval),
        }
    }
}

#[derive(Debug, Clone, clap::Parser)]
pub struct Args {
    #[arg(short, long, help = "Print version information and exit")]
    pub version: bool,

    #[arg(short, long, help = "Enable debug logging")]
    pub debug: bool,

    #[arg(long, help = "Hide diff panel, show only file tree")]
    pub no_diff: bool,

    #[arg(long, help = "Command to run in monitor pane")]
    pub monitor_command: Option<String>,

    #[arg(long, help = "Interval in seconds for monitor command refresh")]
    pub monitor_interval: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.debug);
        assert!(!config.no_diff);
        assert!(config.monitor_command.is_none());
        assert!(config.monitor_interval.is_none());
    }

    #[test]
    fn test_config_new() {
        let config = Config::default();
        assert!(!config.debug);
        assert!(!config.no_diff);
        assert!(config.monitor_command.is_none());
        assert!(config.monitor_interval.is_none());
    }

    #[test]
    fn test_merge_with_args() {
        let mut config = Config::default();
        config.debug = true;
        config.monitor_command = Some("echo test".to_string());

        let mut args = Args::parse_from(&["grw", "--no-diff", "--monitor-interval", "10"]);
        args.debug = false; // Should be overridden by config
        args.monitor_command = None; // Should use config value

        let merged = config.merge_with_args(&args);

        assert!(merged.debug); // From config
        assert!(merged.no_diff); // From args
        assert_eq!(merged.monitor_command, Some("echo test".to_string())); // From config
        assert_eq!(merged.monitor_interval, Some(10)); // From args
    }

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from(&[
            "grw",
            "--debug",
            "--no-diff",
            "--monitor-command",
            "ls -la",
            "--monitor-interval",
            "5",
        ]);

        assert!(args.debug);
        assert!(args.no_diff);
        assert_eq!(args.monitor_command, Some("ls -la".to_string()));
        assert_eq!(args.monitor_interval, Some(5));
    }

    #[test]
    fn test_args_parsing_minimal() {
        let args = Args::parse_from(&["grw"]);

        assert!(!args.debug);
        assert!(!args.no_diff);
        assert!(args.monitor_command.is_none());
        assert!(args.monitor_interval.is_none());
    }
}
