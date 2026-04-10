use serde::{Deserialize, Serialize};

use crate::{ClassifierTrigger, TriggerPhase, TurnFilter};

// ---------------------------------------------------------------------------
// Config types — matches the .json schema
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassifierType {
    Ml,
    Llm,
    Cli,
}

fn default_classifier_type() -> ClassifierType {
    ClassifierType::Ml
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifierConfig {
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub trigger: ConfigTrigger,
    #[serde(rename = "type", default = "default_classifier_type")]
    pub classifier_type: ClassifierType,
    #[serde(default)]
    pub runtime: serde_json::Value,
    pub labels: Vec<LabelConfig>,
}

/// Object form of trigger config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerConfig {
    pub on: TriggerPhase,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_error: Option<bool>,
}

/// Deserializes from either a string or an object for backward compatibility.
/// String: `"on_complete"` / `"incremental"` / `"both"`
/// Object: `{ "on": "incremental", "tools": ["Bash"], "tool_error": true }`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigTrigger {
    Simple(TriggerPhase),
    Rich(TriggerConfig),
}

impl From<ConfigTrigger> for ClassifierTrigger {
    fn from(t: ConfigTrigger) -> Self {
        match t {
            ConfigTrigger::Simple(phase) => ClassifierTrigger::new(phase, TurnFilter::default()),
            ConfigTrigger::Rich(cfg) => ClassifierTrigger::new(
                cfg.on,
                TurnFilter {
                    tools: cfg.tools,
                    tool_error: cfg.tool_error,
                },
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelConfig {
    pub label: String,
    pub notification: NotificationConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationConfig {
    pub message: String,
}
