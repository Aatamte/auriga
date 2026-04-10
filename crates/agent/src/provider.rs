use orchestrator_types::{
    AgentConfig, CommandSpec, GenerateError, GenerateRequest, GenerateResponse,
};

/// A provider that can execute LLM generation requests.
///
/// Implementations handle the specifics of one LLM API
/// (Anthropic, OpenAI, Google, etc.). The trait is deliberately
/// minimal: one method for generation, one for building a CLI command.
///
/// A provider is constructed with its config once and reused for
/// many requests. It holds API keys, HTTP clients, base URLs, etc.
pub trait Provider: Send + Sync {
    /// The provider's identifier (e.g. "claude", "openai", "gemini").
    fn name(&self) -> &str;

    /// Execute a single generation request and return the response.
    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, GenerateError>;

    /// Build a command specification for native CLI mode.
    fn build_command(&self, config: &AgentConfig) -> Option<CommandSpec>;
}
