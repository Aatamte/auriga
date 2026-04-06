/// Specification for spawning a native CLI agent process.
/// The provider builds this; the app layer passes it to PtyHandle.
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// The executable name or path (e.g. "claude", "codex", "gemini-cli").
    pub program: String,
    /// Command line arguments.
    pub args: Vec<String>,
    /// Additional environment variables to set.
    pub env: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_spec_construction() {
        let spec = CommandSpec {
            program: "claude".into(),
            args: vec!["--model".into(), "opus".into(), "-p".into()],
            env: vec![("AGENT_NAME".into(), "test".into())],
        };
        assert_eq!(spec.program, "claude");
        assert_eq!(spec.args.len(), 3);
        assert_eq!(spec.env[0].0, "AGENT_NAME");
    }
}
