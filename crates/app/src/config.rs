use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const DIR_NAME: &str = ".agent-orchestrator";
const CONFIG_FILE: &str = "config.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub mcp_port: u16,
    #[serde(default)]
    pub disabled_classifiers: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mcp_port: 7850,
            disabled_classifiers: Vec::new(),
        }
    }
}

pub fn dir_path() -> PathBuf {
    PathBuf::from(DIR_NAME)
}

/// Ensures `.agent-orchestrator/` exists with default files.
/// Returns the loaded config.
pub fn init() -> Result<Config> {
    let dir = dir_path();
    fs::create_dir_all(&dir)?;

    write_default_layout(&dir)?;
    let config = load_or_create_config(&dir)?;

    Ok(config)
}

fn write_default_layout(dir: &Path) -> Result<()> {
    let path = dir.join("layout.json");
    if path.exists() {
        return Ok(());
    }
    let grid = orchestrator_grid::Grid::default();
    let json = serde_json::to_string_pretty(&grid)?;
    fs::write(path, json)?;
    Ok(())
}

/// Save config to disk.
pub fn save(config: &Config) -> Result<()> {
    let path = dir_path().join(CONFIG_FILE);
    let json = serde_json::to_string_pretty(config)?;
    fs::write(path, json)?;
    Ok(())
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
        let config = Config {
            mcp_port: 9000,
            disabled_classifiers: vec![],
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: Config = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mcp_port, 9000);
    }
}
