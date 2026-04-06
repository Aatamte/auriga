use std::fmt;

use crate::command::CommandSpec;
use crate::config::AgentConfig;
use crate::message::{GenerateRequest, GenerateResponse};

/// Errors that can occur during LLM generation.
#[derive(Debug)]
pub enum GenerateError {
    /// The provider returned an HTTP error.
    Api { status: u16, message: String },
    /// Rate limited. Contains retry-after in seconds if available.
    RateLimited { retry_after: Option<u64> },
    /// Request or response serialization/deserialization failed.
    Serialization(String),
    /// Network or I/O error.
    Network(String),
    /// The provider rejected the request due to content policy.
    ContentFiltered(String),
    /// Context window exceeded.
    ContextLengthExceeded {
        max_tokens: u64,
        requested_tokens: u64,
    },
    /// Authentication failed.
    Authentication(String),
    /// Any other provider error.
    Other(String),
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Api { status, message } => write!(f, "API error {}: {}", status, message),
            Self::RateLimited { retry_after } => match retry_after {
                Some(secs) => write!(f, "rate limited, retry after {}s", secs),
                None => write!(f, "rate limited"),
            },
            Self::Serialization(msg) => write!(f, "serialization error: {}", msg),
            Self::Network(msg) => write!(f, "network error: {}", msg),
            Self::ContentFiltered(msg) => write!(f, "content filtered: {}", msg),
            Self::ContextLengthExceeded {
                max_tokens,
                requested_tokens,
            } => write!(
                f,
                "context length exceeded: requested {} tokens, max {}",
                requested_tokens, max_tokens
            ),
            Self::Authentication(msg) => write!(f, "authentication error: {}", msg),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for GenerateError {}

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
    ///
    /// This is the foundational primitive. Managed agent loops call
    /// this repeatedly. Classifiers call this once. The orchestrator
    /// never calls the LLM API directly — it always goes through
    /// a Provider.
    ///
    /// This method is synchronous and blocking. The caller is
    /// responsible for running it on a background thread if needed.
    fn generate(&self, request: &GenerateRequest) -> Result<GenerateResponse, GenerateError>;

    /// Build a command specification for native CLI mode.
    ///
    /// Given the agent config, returns the program + args + env
    /// needed to spawn the provider's CLI tool as a PTY process.
    /// Returns None if this provider does not support native CLI mode.
    fn build_command(&self, config: &AgentConfig) -> Option<CommandSpec>;
}
