use std::fmt;

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
