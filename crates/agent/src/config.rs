use serde::{Deserialize, Serialize};

/// How the orchestrator interacts with the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMode {
    /// Single request/response. No session. No tool loop.
    Generate,
    /// Orchestrator-managed agent loop with tool execution.
    Managed,
    /// Spawn native CLI process. Orchestrator observes but does not control.
    NativeCli,
}

/// Describes how to create and configure an LLM agent.
/// Serializable for persistence as reusable templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Human-readable name for this config (e.g. "code-review", "classifier-llm").
    pub name: String,
    /// Which provider to use (e.g. "claude", "openai", "gemini").
    pub provider: String,
    /// The model identifier (e.g. "claude-sonnet-4-20250514", "gpt-4o").
    pub model: String,
    /// Maximum tokens to generate per response.
    pub max_tokens: u32,
    /// System prompt prepended to every conversation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// Temperature (0.0 - 1.0). None means provider default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// How the agent interacts with the orchestrator.
    pub mode: AgentMode,
    /// Provider-specific configuration (opaque JSON).
    /// Parsed by the provider implementation.
    #[serde(default = "default_provider_config")]
    pub provider_config: serde_json::Value,
}

fn default_provider_config() -> serde_json::Value {
    serde_json::Value::Object(serde_json::Map::new())
}

/// Builder for composing a system prompt from multiple sections.
/// Sections are joined with `\n\n---\n\n`. Empty strings are ignored.
pub struct SystemPromptBuilder {
    sections: Vec<String>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            sections: Vec::new(),
        }
    }

    /// Append a named section. Empty content is skipped.
    pub fn section(mut self, content: &str) -> Self {
        if !content.is_empty() {
            self.sections.push(content.to_string());
        }
        self
    }

    /// Append a named section with a header. Empty content is skipped.
    pub fn titled_section(mut self, title: &str, content: &str) -> Self {
        if !content.is_empty() {
            self.sections.push(format!("# {}\n\n{}", title, content));
        }
        self
    }

    pub fn build(self) -> Option<String> {
        if self.sections.is_empty() {
            None
        } else {
            Some(self.sections.join("\n\n---\n\n"))
        }
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_config_round_trips_through_json() {
        let config = AgentConfig {
            name: "test-agent".into(),
            provider: "claude".into(),
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 4096,
            system_prompt: Some("You are helpful.".into()),
            temperature: Some(0.7),
            mode: AgentMode::Generate,
            provider_config: serde_json::json!({"api_key_env": "ANTHROPIC_API_KEY"}),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: AgentConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "test-agent");
        assert_eq!(parsed.provider, "claude");
        assert_eq!(parsed.model, "claude-sonnet-4-20250514");
        assert_eq!(parsed.max_tokens, 4096);
        assert_eq!(parsed.system_prompt.as_deref(), Some("You are helpful."));
        assert_eq!(parsed.temperature, Some(0.7));
        assert_eq!(parsed.mode, AgentMode::Generate);
    }

    #[test]
    fn agent_mode_serializes_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&AgentMode::NativeCli).unwrap(),
            "\"native_cli\""
        );
        assert_eq!(
            serde_json::to_string(&AgentMode::Generate).unwrap(),
            "\"generate\""
        );
        assert_eq!(
            serde_json::to_string(&AgentMode::Managed).unwrap(),
            "\"managed\""
        );
    }

    #[test]
    fn missing_optional_fields_get_defaults() {
        let json = r#"{
            "name": "minimal",
            "provider": "openai",
            "model": "gpt-4o",
            "max_tokens": 1024,
            "mode": "generate"
        }"#;
        let config: AgentConfig = serde_json::from_str(json).unwrap();
        assert!(config.system_prompt.is_none());
        assert!(config.temperature.is_none());
        assert!(config.provider_config.is_object());
    }

    #[test]
    fn prompt_builder_empty_is_none() {
        assert!(SystemPromptBuilder::new().build().is_none());
    }

    #[test]
    fn prompt_builder_skips_empty_sections() {
        let result = SystemPromptBuilder::new()
            .section("")
            .section("")
            .build();
        assert!(result.is_none());
    }

    #[test]
    fn prompt_builder_single_section() {
        let result = SystemPromptBuilder::new()
            .section("You are helpful.")
            .build();
        assert_eq!(result.unwrap(), "You are helpful.");
    }

    #[test]
    fn prompt_builder_joins_with_separator() {
        let result = SystemPromptBuilder::new()
            .section("Be helpful.")
            .section("Be concise.")
            .build()
            .unwrap();
        assert_eq!(result, "Be helpful.\n\n---\n\nBe concise.");
    }

    #[test]
    fn prompt_builder_titled_section() {
        let result = SystemPromptBuilder::new()
            .section("Base prompt.")
            .titled_section("Repository Context", "This is a Rust project.")
            .build()
            .unwrap();
        assert!(result.contains("# Repository Context"));
        assert!(result.contains("This is a Rust project."));
        assert!(result.contains("---"));
    }

    #[test]
    fn prompt_builder_titled_section_skips_empty() {
        let result = SystemPromptBuilder::new()
            .titled_section("Context", "")
            .build();
        assert!(result.is_none());
    }
}
