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
            _ => Err(format!("Invalid theme: {s}. Must be 'dark' or 'light'")),
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
            _ => Err(format!("Invalid LLM provider: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlmConfig {
    pub provider: Option<LlmProvider>,
    pub model: Option<String>,
    pub summary_model: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub advice_model: Option<String>,
}

impl LlmConfig {
    /// Get the model to use for summary generation, falling back to default model
    pub fn get_summary_model(&self) -> String {
        self.summary_model
            .clone()
            .or_else(|| self.model.clone())
            .unwrap_or_else(|| "gpt-4o-mini".to_string())
    }

    /// Get the model to use for advice generation, falling back to default model
    pub fn get_advice_model(&self) -> String {
        self.advice_model
            .clone()
            .or_else(|| self.model.clone())
            .unwrap_or_else(|| "gpt-4o-mini".to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AdviceConfig {
    pub enabled: Option<bool>,
    pub advice_model: Option<String>,
    pub max_improvements: Option<usize>,
    pub chat_history_limit: Option<usize>,
    pub timeout_seconds: Option<u64>,
    pub context_lines: Option<usize>,
}

impl AdviceConfig {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub debug: Option<bool>,
    pub no_diff: Option<bool>,
    pub hide_changed_files_pane: Option<bool>,
    pub monitor_command: Option<String>,
    pub monitor_interval: Option<u64>,
    pub theme: Option<Theme>,
    pub llm: Option<LlmConfig>,
    pub advice: Option<AdviceConfig>,
    pub commit_history_limit: Option<usize>,
    pub commit_cache_size: Option<usize>,
    pub summary_preload_enabled: Option<bool>,
    pub summary_preload_count: Option<usize>,
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

    /// Get the commit history limit with a sensible default
    pub fn get_commit_history_limit(&self) -> usize {
        self.commit_history_limit.unwrap_or(100)
    }

    /// Get the commit cache size with a sensible default
    pub fn get_commit_cache_size(&self) -> usize {
        self.commit_cache_size.unwrap_or(200)
    }

    /// Get the summary preload configuration
    pub fn get_summary_preload_config(&self) -> crate::git::PreloadConfig {
        crate::git::PreloadConfig {
            enabled: self.summary_preload_enabled.unwrap_or(true),
            count: self.summary_preload_count.unwrap_or(5),
        }
    }

    /// Get shared state configuration settings
    pub fn get_shared_state_config(&self) -> SharedStateConfig {
        SharedStateConfig {
            commit_cache_size: self.get_commit_cache_size(),
            commit_history_limit: self.get_commit_history_limit(),
            summary_preload_enabled: self.summary_preload_enabled.unwrap_or(true),
            summary_preload_count: self.summary_preload_count.unwrap_or(5),
            cache_cleanup_interval: 300, // 5 minutes
            stale_task_threshold: 3600,  // 1 hour
        }
    }
}

/// Configuration for shared state components
#[derive(Debug, Clone)]
pub struct SharedStateConfig {
    pub commit_cache_size: usize,
    pub commit_history_limit: usize,
    // TODO: These fields are reserved for future use
    #[allow(dead_code)]
    pub summary_preload_enabled: bool,
    #[allow(dead_code)]
    pub summary_preload_count: usize,
    pub cache_cleanup_interval: u64,
    pub stale_task_threshold: u64,
}

impl Config {
    fn get_config_path() -> PathBuf {
        config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("grw")
            .join("config.json")
    }

    pub fn merge_with_args(&self, args: &Args) -> Self {
        let llm_config = self.llm.clone().unwrap_or_default();
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
            llm: Some(LlmConfig {
                provider: args.llm_provider.clone().or(llm_config.provider),
                model: args.llm_model.clone().or(llm_config.model),
                summary_model: args.llm_summary_model.clone().or(llm_config.summary_model),
                api_key: args.llm_api_key.clone().or(llm_config.api_key),
                base_url: args.llm_base_url.clone().or(llm_config.base_url),
                advice_model: args.llm_advice_model.clone().or(llm_config.advice_model),
            }),
            advice: self.advice.clone(),
            commit_history_limit: args.commit_history_limit.or(self.commit_history_limit),
            commit_cache_size: args.commit_cache_size.or(self.commit_cache_size),
            summary_preload_enabled: args
                .summary_preload_enabled
                .or(self.summary_preload_enabled),
            summary_preload_count: args.summary_preload_count.or(self.summary_preload_count),
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

    #[arg(long, help = "LLM provider to use for advice (e.g., openai)")]
    pub llm_provider: Option<LlmProvider>,

    #[arg(long, help = "LLM model to use for advice")]
    pub llm_model: Option<String>,

    #[arg(
        long,
        help = "LLM model to use specifically for commit summary generation"
    )]
    pub llm_summary_model: Option<String>,

    #[arg(long, help = "LLM model to use specifically for advice generation")]
    pub llm_advice_model: Option<String>,

    #[arg(long, help = "API key for the LLM provider")]
    pub llm_api_key: Option<String>,

    #[arg(long, help = "Base URL for the LLM provider")]
    pub llm_base_url: Option<String>,

    #[arg(
        long,
        help = "Maximum number of commits to load in commit picker (default: 100)"
    )]
    pub commit_history_limit: Option<usize>,

    #[arg(long, help = "Maximum number of commits to cache (default: 200)")]
    pub commit_cache_size: Option<usize>,

    #[arg(long, help = "Enable summary pre-loading (default: true)")]
    pub summary_preload_enabled: Option<bool>,

    #[arg(long, help = "Number of summaries to pre-load (default: 5)")]
    pub summary_preload_count: Option<usize>,
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
    fn test_commit_cache_size_config() {
        let config = Config {
            commit_cache_size: Some(300),
            ..Default::default()
        };
        assert_eq!(config.get_commit_cache_size(), 300);

        let config_default = Config::default();
        assert_eq!(config_default.get_commit_cache_size(), 200); // Default value
    }

    #[test]
    fn test_merge_with_args_commit_settings() {
        let config = Config {
            commit_history_limit: Some(50),
            commit_cache_size: Some(100),
            ..Default::default()
        };

        let args = Args::parse_from([
            "grw",
            "--commit-history-limit",
            "200",
            "--commit-cache-size",
            "400",
        ]);

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.commit_history_limit, Some(200)); // From args
        assert_eq!(merged.commit_cache_size, Some(400)); // From args
    }

    #[test]
    fn test_merge_with_args_commit_settings_from_config() {
        let config = Config {
            commit_history_limit: Some(75),
            commit_cache_size: Some(150),
            ..Default::default()
        };

        let args = Args::parse_from(["grw"]); // No commit settings specified

        let merged = config.merge_with_args(&args);

        assert_eq!(merged.commit_history_limit, Some(75)); // From config
        assert_eq!(merged.commit_cache_size, Some(150)); // From config
    }

    #[test]
    fn test_llm_config_model_fallback() {
        // Test summary model fallback to general model
        let config = LlmConfig {
            model: Some("gpt-3.5-turbo".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_summary_model(), "gpt-3.5-turbo");

        // Test fallback to default when nothing is configured
        let config = LlmConfig::default();
        assert_eq!(config.get_summary_model(), "gpt-4o-mini");

        // Test specific model overrides general model
        let config = LlmConfig {
            model: Some("gpt-3.5-turbo".to_string()),
            summary_model: Some("gpt-4o-mini".to_string()),
            ..Default::default()
        };
        assert_eq!(config.get_summary_model(), "gpt-4o-mini");
    }

    #[test]
    fn test_merge_with_args_llm_models() {
        let config = Config {
            llm: Some(LlmConfig {
                model: Some("gpt-3.5-turbo".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let args = Args::parse_from([
            "grw",
            "--llm-model",
            "gpt-4-turbo",
            "--llm-summary-model",
            "gpt-4o-mini",
        ]);

        let merged = config.merge_with_args(&args);
        let llm_config = merged.llm.unwrap();

        assert_eq!(llm_config.model, Some("gpt-4-turbo".to_string())); // From args (CLI takes precedence)
        assert_eq!(llm_config.summary_model, Some("gpt-4o-mini".to_string())); // From args
    }

    #[test]
    fn test_merge_with_args_llm_models_from_config() {
        let config = Config {
            llm: Some(LlmConfig {
                summary_model: Some("gpt-4o-mini".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let args = Args::parse_from(["grw"]); // No LLM model args specified

        let merged = config.merge_with_args(&args);
        let llm_config = merged.llm.unwrap();

        assert_eq!(llm_config.summary_model, Some("gpt-4o-mini".to_string())); // From config
    }
}
