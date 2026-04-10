pub mod claude;
pub mod codex;

use crate::provider::Provider;

/// Resolve a provider by its name (`"claude"`, `"codex"`).
/// Panics on unknown names — callers are expected to validate input.
pub fn resolve(name: &str) -> Box<dyn Provider> {
    match name {
        "claude" => Box::new(claude::ClaudeProvider),
        "codex" => Box::new(codex::CodexProvider),
        other => panic!("unknown provider: {}", other),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_claude() {
        assert_eq!(resolve("claude").name(), "claude");
    }

    #[test]
    fn resolve_codex() {
        assert_eq!(resolve("codex").name(), "codex");
    }

    #[test]
    #[should_panic(expected = "unknown provider")]
    fn resolve_unknown_panics() {
        let _ = resolve("gemini");
    }
}
