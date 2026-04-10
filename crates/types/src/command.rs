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
