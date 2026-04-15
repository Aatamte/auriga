use anyhow::Result;
use auriga_core::ClaudeCliConfig;
use auriga_grid::Grid;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const DIR_NAME: &str = ".auriga";
const CONFIG_FILE: &str = "settings.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mcp_port: u16,
    #[serde(default, deserialize_with = "deserialize_layout")]
    pub layout: Grid,
    #[serde(default = "default_provider")]
    pub default_provider: String,
    #[serde(default = "default_true")]
    pub claude_enabled: bool,
    #[serde(default = "default_display_mode")]
    pub display_mode: String,
    #[serde(default = "default_font_size")]
    pub font_size: u16,
    /// Claude CLI configuration used when spawning new Claude agents.
    #[serde(default)]
    pub claude: ClaudeCliConfig,
}

fn default_font_size() -> u16 {
    10
}

fn default_provider() -> String {
    "claude".into()
}

fn default_true() -> bool {
    true
}

fn default_display_mode() -> String {
    "provider".into()
}

/// Deserialize layout, falling back to default if the JSON is invalid (e.g. stale widget ids).
fn deserialize_layout<'de, D>(deserializer: D) -> Result<Grid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<serde_json::Value>::deserialize(deserializer)?;
    match value {
        Some(v) => match serde_json::from_value(v) {
            Ok(grid) => Ok(grid),
            Err(_) => Ok(Grid::default()),
        },
        None => Ok(Grid::default()),
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mcp_port: 7850,
            layout: Grid::default(),
            default_provider: default_provider(),
            claude_enabled: true,
            display_mode: default_display_mode(),
            font_size: default_font_size(),
            claude: ClaudeCliConfig::default(),
        }
    }
}

pub fn dir_path() -> PathBuf {
    PathBuf::from(DIR_NAME)
}

pub fn file_path() -> PathBuf {
    dir_path().join(CONFIG_FILE)
}

/// Ensures `.auriga/` exists with default files.
/// Returns the loaded config.
pub fn init() -> Result<Config> {
    let dir = dir_path();
    fs::create_dir_all(&dir)?;

    let config = load_or_create_config(&dir)?;

    // Ensure default prompts exist
    let prompts_dir = dir.join("prompts");
    fs::create_dir_all(&prompts_dir)?;
    create_default_prompt(
        &prompts_dir,
        "coding-assistant",
        include_str!("defaults/coding-assistant.json"),
    )?;

    Ok(config)
}

fn create_default_prompt(dir: &Path, name: &str, content: &str) -> Result<()> {
    let path = dir.join(format!("{}.json", name));
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Save config to disk.
pub fn save(config: &Config) -> Result<()> {
    let json = serde_json::to_string_pretty(config)?;
    fs::write(file_path(), json)?;
    Ok(())
}

pub fn load() -> Result<Config> {
    let path = file_path();
    let contents = fs::read_to_string(path)?;
    let config: Config = serde_json::from_str(&contents)?;
    Ok(config)
}

pub fn modified_at() -> Option<SystemTime> {
    fs::metadata(file_path()).and_then(|m| m.modified()).ok()
}

/// Load the Claude CLI config as a JSON value for `AgentConfig.provider_config`.
pub fn load_claude_config() -> serde_json::Value {
    match load() {
        Ok(config) => serde_json::to_value(&config.claude).unwrap_or_default(),
        Err(_) => serde_json::json!({}),
    }
}

fn load_or_create_config(dir: &Path) -> Result<Config> {
    let path = dir.join(CONFIG_FILE);
    if path.exists() {
        let contents = fs::read_to_string(&path)?;
        let config: Config = serde_json::from_str(&contents)?;
        return Ok(config);
    }
    let config = Config::default();
    let json = serde_json::to_string_pretty(&config)?;
    fs::write(path, json)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_port_7850() {
        let config = Config::default();
        assert_eq!(config.mcp_port, 7850);
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = Config::default();
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mcp_port, config.mcp_port);
        assert_eq!(parsed.layout.columns, config.layout.columns);
        assert_eq!(parsed.default_provider, config.default_provider);
        assert_eq!(parsed.claude_enabled, config.claude_enabled);
        assert_eq!(parsed.display_mode, config.display_mode);
        assert_eq!(parsed.font_size, config.font_size);
    }

    #[test]
    fn config_without_layout_gets_default() {
        let json = r#"{"mcp_port": 7850}"#;
        let config: Config = serde_json::from_str(json).unwrap();
        assert_eq!(config.layout.columns, 12);
        assert!(!config.layout.rows.is_empty());
    }
}
