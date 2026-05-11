use dirs::config_dir;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Custom,
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            "custom" => Ok(Theme::Custom),
            _ => Err(serde::de::Error::custom(format!(
                "invalid theme: {s}, expected 'dark', 'light', or 'custom'"
            ))),
        }
    }
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            _ => Err(format!(
                "invalid theme: {s}, expected 'dark' or 'light'. Use config file for custom theme."
            )),
        }
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Dark => write!(f, "dark"),
            Theme::Light => write!(f, "light"),
            Theme::Custom => write!(f, "custom"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CustomTheme {
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub primary: Option<String>,
    pub secondary: Option<String>,
    pub error: Option<String>,
    pub highlight: Option<String>,
    pub border: Option<String>,
    pub directory: Option<String>,
    pub added: Option<String>,
    pub removed: Option<String>,
    pub unchanged: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub debug: Option<bool>,
    pub no_diff: Option<bool>,
    pub hide_changed_files_pane: Option<bool>,
    pub monitor_command: Option<String>,
    pub monitor_interval: Option<u64>,
    pub theme: Option<Theme>,
    pub custom_theme: Option<CustomTheme>,
    pub commit_history_limit: Option<usize>,
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

    pub fn get_commit_history_limit(&self) -> usize {
        self.commit_history_limit.unwrap_or(100)
    }
}

impl Config {
    fn get_config_path() -> PathBuf {
        config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("grw")
            .join("config.json")
    }

    pub fn merge_with_args(&self, args: &Args) -> Self {
        Self {
            debug: if args.debug { Some(true) } else { self.debug },
            no_diff: if args.no_diff {
                Some(true)
            } else {
                self.no_diff
            },
            hide_changed_files_pane: if args.hide_changed_files_pane {
                Some(true)
            } else {
                self.hide_changed_files_pane
            },
            monitor_command: args
                .monitor_command
                .clone()
                .or_else(|| self.monitor_command.clone()),
            monitor_interval: args.monitor_interval.or(self.monitor_interval),
            theme: args.theme.clone().or_else(|| self.theme.clone()),
            custom_theme: self.custom_theme.clone(),
            commit_history_limit: args.commit_history_limit.or(self.commit_history_limit),
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

    #[arg(long, help = "Hide changed files pane, show only diff")]
    pub hide_changed_files_pane: bool,

    #[arg(long, help = "Command to run in monitor pane")]
    pub monitor_command: Option<String>,

    #[arg(long, help = "Interval in seconds for monitor command refresh")]
    pub monitor_interval: Option<u64>,

    #[arg(long, help = "Theme to use (dark or light)")]
    pub theme: Option<Theme>,

    #[arg(
        long,
        help = "Maximum number of commits to load in commit picker (default: 100)"
    )]
    pub commit_history_limit: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.debug, None);
        assert_eq!(config.no_diff, None);
        assert!(config.monitor_command.is_none());
        assert!(config.monitor_interval.is_none());
        assert_eq!(config.theme, None);
    }

    #[test]
    fn test_config_new() {
        let config = Config::default();
        assert_eq!(config.debug, None);
        assert_eq!(config.no_diff, None);
        assert!(config.monitor_command.is_none());
        assert!(config.monitor_interval.is_none());
        assert_eq!(config.theme, None);
    }

    #[test]
    fn test_merge_with_args() {
        let config = Config {
            debug: Some(true),
            monitor_command: Some("echo test".to_string()),
            theme: Some(Theme::Light),
            ..Default::default()
        };

        let args = Args::parse_from([
            "grw",
            "--debug", // CLI args take precedence
            "--no-diff",
            "--monitor-interval",
            "10",
            "--theme",
            "dark",
        ]);

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.debug, Some(true)); // From args (CLI takes precedence)
        assert_eq!(merged.no_diff, Some(true)); // From args
        assert_eq!(merged.monitor_command, Some("echo test".to_string())); // From config
        assert_eq!(merged.monitor_interval, Some(10)); // From args
        assert_eq!(merged.theme, Some(Theme::Dark)); // From args (CLI takes precedence)
    }

    #[test]
    fn test_merge_with_args_theme_from_config() {
        let config = Config {
            theme: Some(Theme::Light),
            ..Default::default()
        };

        let args = Args::parse_from(["grw"]); // No theme specified

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.theme, Some(Theme::Light)); // From config
    }

    #[test]
    fn test_merge_with_args_hide_changed_files_pane() {
        let mut config = Config::default();
        let args = Args::parse_from(["grw", "--hide-changed-files-pane"]);
        let merged = config.merge_with_args(&args);
        assert_eq!(merged.hide_changed_files_pane, Some(true));

        config.hide_changed_files_pane = Some(false);
        let merged = config.merge_with_args(&args);
        assert_eq!(merged.hide_changed_files_pane, Some(true)); // CLI overrides config

        config.hide_changed_files_pane = Some(true);
        let args = Args::parse_from(["grw"]);
        let merged = config.merge_with_args(&args);
        assert_eq!(merged.hide_changed_files_pane, Some(true));
    }

    #[test]
    fn test_args_parsing() {
        let args = Args::parse_from([
            "grw",
            "--debug",
            "--no-diff",
            "--monitor-command",
            "ls -la",
            "--monitor-interval",
            "5",
            "--theme",
            "light",
        ]);

        assert!(args.debug);
        assert!(args.no_diff);
        assert_eq!(args.monitor_command, Some("ls -la".to_string()));
        assert_eq!(args.monitor_interval, Some(5));
        assert_eq!(args.theme, Some(Theme::Light));
    }

    #[test]
    fn test_args_parsing_minimal() {
        let args = Args::parse_from(["grw"]);

        assert!(!args.debug);
        assert!(!args.no_diff);
        assert!(args.monitor_command.is_none());
        assert!(args.monitor_interval.is_none());
        assert!(args.theme.is_none());
    }

    #[test]
    fn test_theme_from_str() {
        assert_eq!(Theme::from_str("dark").unwrap(), Theme::Dark);
        assert_eq!(Theme::from_str("light").unwrap(), Theme::Light);
        assert_eq!(Theme::from_str("DARK").unwrap(), Theme::Dark);
        assert_eq!(Theme::from_str("LIGHT").unwrap(), Theme::Light);
        assert!(Theme::from_str("invalid").is_err());
    }

    #[test]
    fn test_theme_display() {
        assert_eq!(Theme::Dark.to_string(), "dark");
        assert_eq!(Theme::Light.to_string(), "light");
    }

    #[test]
    fn test_args_parsing_with_theme() {
        let args = Args::parse_from(["grw", "--theme", "light"]);
        assert_eq!(args.theme, Some(Theme::Light));

        let args = Args::parse_from(["grw", "--theme", "dark"]);
        assert_eq!(args.theme, Some(Theme::Dark));
    }

    #[test]
    fn test_args_parsing_invalid_theme() {
        let result = Args::try_parse_from(["grw", "--theme", "invalid"]);
        assert!(result.is_err(), "Should fail to parse invalid theme");
    }

    #[test]
    fn test_config_deserialize_case_insensitive() {
        let json_dark_upper = r#"{"debug": false, "no_diff": false, "theme": "DARK"}"#;
        let json_dark_lower = r#"{"debug": false, "no_diff": false, "theme": "dark"}"#;
        let json_dark_mixed = r#"{"debug": false, "no_diff": false, "theme": "DaRk"}"#;
        let json_light_upper = r#"{"debug": false, "no_diff": false, "theme": "LIGHT"}"#;
        let json_light_lower = r#"{"debug": false, "no_diff": false, "theme": "light"}"#;
        let json_light_mixed = r#"{"debug": false, "no_diff": false, "theme": "LiGhT"}"#;
        let json_no_theme = r#"{"debug": false, "no_diff": false}"#;

        let config_dark_upper: Config = serde_json::from_str(json_dark_upper).unwrap();
        let config_dark_lower: Config = serde_json::from_str(json_dark_lower).unwrap();
        let config_dark_mixed: Config = serde_json::from_str(json_dark_mixed).unwrap();
        let config_light_upper: Config = serde_json::from_str(json_light_upper).unwrap();
        let config_light_lower: Config = serde_json::from_str(json_light_lower).unwrap();
        let config_light_mixed: Config = serde_json::from_str(json_light_mixed).unwrap();
        let config_no_theme: Config = serde_json::from_str(json_no_theme).unwrap();

        assert_eq!(config_dark_upper.debug, Some(false));
        assert_eq!(config_dark_upper.no_diff, Some(false));
        assert_eq!(config_dark_upper.theme, Some(Theme::Dark));
        assert_eq!(config_dark_lower.theme, Some(Theme::Dark));
        assert_eq!(config_dark_mixed.theme, Some(Theme::Dark));
        assert_eq!(config_light_upper.theme, Some(Theme::Light));
        assert_eq!(config_light_lower.theme, Some(Theme::Light));
        assert_eq!(config_light_mixed.theme, Some(Theme::Light));
        assert_eq!(config_no_theme.debug, Some(false));
        assert_eq!(config_no_theme.no_diff, Some(false));
        assert_eq!(config_no_theme.theme, None);
    }

    #[test]
    fn test_commit_history_limit_config() {
        let config = Config {
            commit_history_limit: Some(150),
            ..Default::default()
        };
        assert_eq!(config.get_commit_history_limit(), 150);

        let config_default = Config::default();
        assert_eq!(config_default.get_commit_history_limit(), 100); // Default value
    }

    #[test]
    fn test_merge_with_args_commit_settings() {
        let config = Config {
            commit_history_limit: Some(50),
            ..Default::default()
        };

        let args = Args::parse_from(["grw", "--commit-history-limit", "200"]);

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.commit_history_limit, Some(200)); // From args
    }

    #[test]
    fn test_merge_with_args_commit_settings_from_config() {
        let config = Config {
            commit_history_limit: Some(75),
            ..Default::default()
        };

        let args = Args::parse_from(["grw"]); // No commit settings specified

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.commit_history_limit, Some(75)); // From config
    }
}
