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
