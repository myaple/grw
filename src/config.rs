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
}

impl<'de> Deserialize<'de> for Theme {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Theme::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for Theme {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            _ => Err(format!("Invalid theme: {}. Must be 'dark' or 'light'", s)),
        }
    }
}

impl std::fmt::Display for Theme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Theme::Dark => write!(f, "dark"),
            Theme::Light => write!(f, "light"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum LlmProvider {
    #[default]
    OpenAI,
}

impl FromStr for LlmProvider {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "openai" => Ok(LlmProvider::OpenAI),
            _ => Err(format!("Invalid LLM provider: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub api_key: Option<String>,
    pub interval: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub debug: Option<bool>,
    pub no_diff: Option<bool>,
    pub monitor_command: Option<String>,
    pub monitor_interval: Option<u64>,
    pub theme: Option<Theme>,
    pub llm: Option<LlmConfig>,
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
        let llm_config = self.llm.clone().unwrap_or_default();
        Self {
            debug: Some(args.debug).or(self.debug),
            no_diff: Some(args.no_diff).or(self.no_diff),
            monitor_command: args
                .monitor_command
                .clone()
                .or_else(|| self.monitor_command.clone()),
            monitor_interval: args.monitor_interval.or(self.monitor_interval),
            theme: args.theme.clone().or_else(|| self.theme.clone()),
            llm: Some(LlmConfig {
                provider: args.llm_provider.clone().or(llm_config.provider),
                model: args.llm_model.clone().or(llm_config.model),
                api_key: args.llm_api_key.clone().or(llm_config.api_key),
                interval: args.llm_interval.or(llm_config.interval),
            }),
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

    #[arg(long, help = "Theme to use (dark or light)")]
    pub theme: Option<Theme>,

    #[arg(long, help = "LLM provider to use for advice (e.g., openai)")]
    pub llm_provider: Option<LlmProvider>,

    #[arg(long, help = "LLM model to use for advice")]
    pub llm_model: Option<String>,

    #[arg(long, help = "API key for the LLM provider")]
    pub llm_api_key: Option<String>,

    #[arg(
        long,
        help = "Interval in seconds for LLM advice refresh",
        default_value = "60"
    )]
    pub llm_interval: Option<u64>,
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
        let mut config = Config::default();
        config.debug = Some(true);
        config.monitor_command = Some("echo test".to_string());
        config.theme = Some(Theme::Light);

        let args = Args::parse_from(&[
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
        let mut config = Config::default();
        config.theme = Some(Theme::Light);

        let args = Args::parse_from(&["grw"]); // No theme specified

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.theme, Some(Theme::Light)); // From config
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
        let args = Args::parse_from(&["grw"]);

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
        let args = Args::parse_from(&["grw", "--theme", "light"]);
        assert_eq!(args.theme, Some(Theme::Light));

        let args = Args::parse_from(&["grw", "--theme", "dark"]);
        assert_eq!(args.theme, Some(Theme::Dark));
    }

    #[test]
    fn test_args_parsing_invalid_theme() {
        let result = Args::try_parse_from(&["grw", "--theme", "invalid"]);
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
}
