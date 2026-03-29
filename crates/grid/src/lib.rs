mod layout;

pub use layout::{Cell, CellRect, Grid, Row, Size};

use std::path::PathBuf;

pub fn layout_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".agent-orchestrator")
        .join("layout.json")
}

pub fn load_or_default() -> Grid {
    load_layout().unwrap_or_default()
}

fn load_layout() -> Option<Grid> {
    let path = layout_path();
    let contents = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}
