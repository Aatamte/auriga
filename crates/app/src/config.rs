use anyhow::Result;
use auriga_grid::Grid;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DIR_NAME: &str = ".auriga";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mcp_port: u16,
    #[serde(default)]
    pub disabled_classifiers: Vec<String>,
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
    /// Name of the active Claude CLI preset (matches a file in `presets/`).
    #[serde(default = "default_active_preset")]
    pub active_preset: String,
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

fn default_active_preset() -> String {
    "default".into()
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
            disabled_classifiers: Vec::new(),
            layout: Grid::default(),
            default_provider: default_provider(),
            claude_enabled: true,
            display_mode: default_display_mode(),
            font_size: default_font_size(),
            active_preset: default_active_preset(),
        }
    }
}

pub fn dir_path() -> PathBuf {
    PathBuf::from(DIR_NAME)
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

    // Ensure presets directory exists with default preset
    let presets_dir = dir.join("presets");
    fs::create_dir_all(&presets_dir)?;
    create_default_file(
        &presets_dir,
        "default.json",
        include_str!("defaults/default-preset.json"),
    )?;

    // Ensure context directory exists with default files
    let context_dir = dir.join("context");
    fs::create_dir_all(&context_dir)?;
    create_default_file(&context_dir, "map.md", include_str!("defaults/map.md"))?;
    create_default_file(
        &context_dir,
        "principles.md",
        include_str!("defaults/principles.md"),
    )?;
    create_default_file(
        &context_dir,
        "examples.md",
        include_str!("defaults/examples.md"),
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

fn create_default_file(dir: &Path, name: &str, content: &str) -> Result<()> {
    let path = dir.join(name);
    if !path.exists() {
        fs::write(path, content)?;
    }
    Ok(())
}

/// Save config to disk.
pub fn save(config: &Config) -> Result<()> {
    let path = dir_path().join(CONFIG_FILE);
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
}

/// Load all presets from `.auriga/presets/`.
pub fn load_presets() -> Vec<auriga_core::ClaudePreset> {
    let presets_dir = dir_path().join("presets");
    let Ok(entries) = fs::read_dir(&presets_dir) else {
        return Vec::new();
    };

    let mut presets = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(preset) = serde_json::from_str::<auriga_core::ClaudePreset>(&contents) else {
            continue;
        };
        presets.push(preset);
    }
    presets
}

/// Load the active preset from disk.
pub fn load_active_preset() -> Option<auriga_core::ClaudePreset> {
    let config_path = dir_path().join(CONFIG_FILE);
    let contents = fs::read_to_string(&config_path).ok()?;
    let config: Config = serde_json::from_str(&contents).ok()?;
    let path = dir_path()
        .join("presets")
        .join(format!("{}.json", config.active_preset));
    let preset_contents = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&preset_contents).ok()
}

/// Load the active preset's CLI config as a JSON value for `AgentConfig.provider_config`.
pub fn load_active_provider_config() -> serde_json::Value {
    match load_active_preset() {
        Some(preset) => serde_json::to_value(&preset.config).unwrap_or_default(),
        None => serde_json::json!({}),
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
