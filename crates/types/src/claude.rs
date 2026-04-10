use serde::{Deserialize, Serialize};

/// Permission mode for the Claude CLI session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionMode {
    Default,
    AcceptEdits,
    Plan,
    Auto,
    DontAsk,
    BypassPermissions,
}

impl PermissionMode {
    fn as_cli_str(&self) -> &str {
        match self {
            Self::Default => "default",
            Self::AcceptEdits => "acceptEdits",
            Self::Plan => "plan",
            Self::Auto => "auto",
            Self::DontAsk => "dontAsk",
            Self::BypassPermissions => "bypassPermissions",
        }
    }
}

/// Output format for non-interactive (print) mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFormat {
    Text,
    Json,
    StreamJson,
}

impl OutputFormat {
    fn as_cli_str(&self) -> &str {
        match self {
            Self::Text => "text",
            Self::Json => "json",
            Self::StreamJson => "stream-json",
        }
    }
}

/// Effort level for adaptive reasoning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EffortLevel {
    Low,
    Medium,
    High,
    Max,
}

impl EffortLevel {
    fn as_cli_str(&self) -> &str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }
}

/// Domain model for the Claude Code CLI configuration.
/// Maps 1:1 to `claude --help` flags. Serializable for persistence
/// in AgentConfig.provider_config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ClaudeCliConfig {
    // --- Session identity ---
    /// Session name (--name)
    #[serde(rename = "session_name")]
    pub name: Option<String>,
    /// Resume a session by ID (--resume)
    pub resume: Option<String>,
    /// Continue most recent conversation (--continue)
    pub continue_session: bool,

    // --- Model & reasoning ---
    /// Model override (--model). If unset, uses AgentConfig.model.
    pub model: Option<String>,
    /// Effort level (--effort)
    pub effort: Option<EffortLevel>,

    // --- Permissions & safety ---
    /// Permission mode (--permission-mode)
    pub permission_mode: Option<PermissionMode>,
    /// Allowed tools (--allowedTools), each entry is one specifier
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    /// Disallowed tools (--disallowedTools)
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    /// Skip all permission checks (--dangerously-skip-permissions)
    pub dangerously_skip_permissions: bool,

    // --- Prompts & context ---
    /// System prompt override (--system-prompt). Overrides AgentConfig.system_prompt when set.
    pub system_prompt: Option<String>,
    /// Append to default system prompt (--append-system-prompt)
    pub append_system_prompt: Option<String>,
    /// Additional directories (--add-dir)
    #[serde(default)]
    pub add_dirs: Vec<String>,

    // --- MCP ---
    /// MCP config file path (--mcp-config)
    #[serde(alias = "mcp_config_path")]
    pub mcp_config: Option<String>,
    /// Only use MCP servers from --mcp-config (--strict-mcp-config)
    pub strict_mcp_config: bool,

    // --- Output (print mode) ---
    /// Output format (--output-format)
    pub output_format: Option<OutputFormat>,
    /// Max budget in USD (--max-budget-usd)
    pub max_budget_usd: Option<f64>,

    // --- Tools & agents ---
    /// Built-in tool list override (--tools). Empty = default set.
    #[serde(default)]
    pub tools: Vec<String>,
    /// Agent for the session (--agent)
    pub agent: Option<String>,
    /// Custom agents JSON (--agents)
    pub agents: Option<String>,

    // --- Worktree & environment ---
    /// Create a git worktree (--worktree)
    pub worktree: Option<String>,
    /// Claude Code settings passed via --settings (serialized to a temp file).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub settings: Option<crate::ClaudeSettings>,

    // --- Behavior flags ---
    /// Bare/minimal mode (--bare)
    pub bare: bool,
    /// Verbose output (--verbose)
    pub verbose: bool,
    /// Disable all slash commands/skills (--disable-slash-commands)
    pub disable_slash_commands: bool,

    // --- Environment variables ---
    /// Extra env vars to pass to the CLI process
    #[serde(default)]
    pub env: Vec<(String, String)>,
}

/// A named, reusable Claude CLI configuration preset.
/// Stored as JSON files in `.agent-orchestrator/presets/`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudePreset {
    /// Display name (e.g. "sandbox", "review-agent").
    pub name: String,
    /// Short description shown in the UI.
    #[serde(default)]
    pub description: String,
    /// The CLI configuration this preset applies.
    #[serde(flatten)]
    pub config: ClaudeCliConfig,
}

impl ClaudeCliConfig {
    /// Convert this config into CLI arguments for the `claude` command.
    pub fn to_args(&self) -> Vec<String> {
        let mut args = Vec::new();

        // Session identity
        if let Some(ref name) = self.name {
            args.extend(["--name".into(), name.clone()]);
        }
        if let Some(ref resume) = self.resume {
            args.extend(["--resume".into(), resume.clone()]);
        }
        if self.continue_session {
            args.push("--continue".into());
        }

        // Model & reasoning
        if let Some(ref model) = self.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(ref effort) = self.effort {
            args.extend(["--effort".into(), effort.as_cli_str().into()]);
        }

        // Permissions & safety
        if let Some(ref perm) = self.permission_mode {
            args.extend(["--permission-mode".into(), perm.as_cli_str().into()]);
        }
        for tool in &self.allowed_tools {
            args.extend(["--allowedTools".into(), tool.clone()]);
        }
        for tool in &self.disallowed_tools {
            args.extend(["--disallowedTools".into(), tool.clone()]);
        }
        if self.dangerously_skip_permissions {
            args.push("--dangerously-skip-permissions".into());
        }

        // Prompts & context
        if let Some(ref prompt) = self.system_prompt {
            args.extend(["--system-prompt".into(), prompt.clone()]);
        }
        if let Some(ref prompt) = self.append_system_prompt {
            args.extend(["--append-system-prompt".into(), prompt.clone()]);
        }
        for dir in &self.add_dirs {
            args.extend(["--add-dir".into(), dir.clone()]);
        }

        // MCP
        if let Some(ref mcp) = self.mcp_config {
            args.extend(["--mcp-config".into(), mcp.clone()]);
        }
        if self.strict_mcp_config {
            args.push("--strict-mcp-config".into());
        }

        // Output (print mode)
        if let Some(ref fmt) = self.output_format {
            args.extend(["--output-format".into(), fmt.as_cli_str().into()]);
        }
        if let Some(budget) = self.max_budget_usd {
            args.extend(["--max-budget-usd".into(), budget.to_string()]);
        }

        // Tools & agents
        if !self.tools.is_empty() {
            args.extend(["--tools".into(), self.tools.join(",")]);
        }
        if let Some(ref agent) = self.agent {
            args.extend(["--agent".into(), agent.clone()]);
        }
        if let Some(ref agents) = self.agents {
            args.extend(["--agents".into(), agents.clone()]);
        }

        // Worktree & environment
        if let Some(ref wt) = self.worktree {
            args.extend(["--worktree".into(), wt.clone()]);
        }
        if let Some(ref settings) = self.settings {
            if let Ok(json) = serde_json::to_string(settings) {
                args.extend(["--settings".into(), json]);
            }
        }

        // Behavior flags
        if self.bare {
            args.push("--bare".into());
        }
        if self.verbose {
            args.push("--verbose".into());
        }
        if self.disable_slash_commands {
            args.push("--disable-slash-commands".into());
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_produces_empty_args() {
        let config = ClaudeCliConfig::default();
        assert!(config.to_args().is_empty());
    }

    #[test]
    fn model_and_effort() {
        let config = ClaudeCliConfig {
            model: Some("opus".into()),
            effort: Some(EffortLevel::High),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(args, vec!["--model", "opus", "--effort", "high"]);
    }

    #[test]
    fn permission_mode_serializes_correctly() {
        let config = ClaudeCliConfig {
            permission_mode: Some(PermissionMode::BypassPermissions),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(args, vec!["--permission-mode", "bypassPermissions"]);
    }

    #[test]
    fn allowed_and_disallowed_tools() {
        let config = ClaudeCliConfig {
            allowed_tools: vec!["Bash(git:*)".into(), "Edit".into()],
            disallowed_tools: vec!["Bash(rm *)".into()],
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(
            args,
            vec![
                "--allowedTools",
                "Bash(git:*)",
                "--allowedTools",
                "Edit",
                "--disallowedTools",
                "Bash(rm *)"
            ]
        );
    }

    #[test]
    fn mcp_config_with_alias() {
        // Old field name should still work via serde alias
        let json = r#"{"mcp_config_path": "/tmp/mcp.json"}"#;
        let config: ClaudeCliConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mcp_config.as_deref(), Some("/tmp/mcp.json"));
        let args = config.to_args();
        assert_eq!(args, vec!["--mcp-config", "/tmp/mcp.json"]);
    }

    #[test]
    fn mcp_config_new_field_name() {
        let json = r#"{"mcp_config": "/tmp/mcp.json"}"#;
        let config: ClaudeCliConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.mcp_config.as_deref(), Some("/tmp/mcp.json"));
    }

    #[test]
    fn system_prompt_and_append() {
        let config = ClaudeCliConfig {
            system_prompt: Some("Be helpful.".into()),
            append_system_prompt: Some("Extra context.".into()),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(
            args,
            vec![
                "--system-prompt",
                "Be helpful.",
                "--append-system-prompt",
                "Extra context."
            ]
        );
    }

    #[test]
    fn add_dirs() {
        let config = ClaudeCliConfig {
            add_dirs: vec!["/tmp/extra".into(), "/home/docs".into()],
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(
            args,
            vec!["--add-dir", "/tmp/extra", "--add-dir", "/home/docs"]
        );
    }

    #[test]
    fn output_format_and_budget() {
        let config = ClaudeCliConfig {
            output_format: Some(OutputFormat::StreamJson),
            max_budget_usd: Some(5.50),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(
            args,
            vec!["--output-format", "stream-json", "--max-budget-usd", "5.5"]
        );
    }

    #[test]
    fn boolean_flags() {
        let config = ClaudeCliConfig {
            bare: true,
            verbose: true,
            dangerously_skip_permissions: true,
            strict_mcp_config: true,
            ..Default::default()
        };
        let args = config.to_args();
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--strict-mcp-config".to_string()));
        assert!(args.contains(&"--bare".to_string()));
        assert!(args.contains(&"--verbose".to_string()));
    }

    #[test]
    fn tools_joined_with_comma() {
        let config = ClaudeCliConfig {
            tools: vec!["Bash".into(), "Edit".into(), "Read".into()],
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(args, vec!["--tools", "Bash,Edit,Read"]);
    }

    #[test]
    fn worktree_arg() {
        let config = ClaudeCliConfig {
            worktree: Some("my-branch".into()),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(args, vec!["--worktree", "my-branch"]);
    }

    #[test]
    fn settings_serializes_to_json_arg() {
        use crate::{ClaudeSettings, PermissionsConfig};
        let config = ClaudeCliConfig {
            settings: Some(ClaudeSettings {
                permissions: Some(PermissionsConfig {
                    allow: vec!["Bash".into(), "Skill(commit)".into()],
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let args = config.to_args();
        assert_eq!(args.len(), 2);
        assert_eq!(args[0], "--settings");
        let parsed: serde_json::Value = serde_json::from_str(&args[1]).unwrap();
        let allow = parsed["permissions"]["allow"].as_array().unwrap();
        assert_eq!(allow.len(), 2);
        assert_eq!(allow[0], "Bash");
        assert_eq!(allow[1], "Skill(commit)");
    }

    #[test]
    fn resume_and_continue() {
        let config = ClaudeCliConfig {
            resume: Some("sess-abc".into()),
            ..Default::default()
        };
        assert_eq!(config.to_args(), vec!["--resume", "sess-abc"]);

        let config2 = ClaudeCliConfig {
            continue_session: true,
            ..Default::default()
        };
        assert_eq!(config2.to_args(), vec!["--continue"]);
    }

    #[test]
    fn full_config_round_trips_through_json() {
        let config = ClaudeCliConfig {
            name: Some("my-session".into()),
            model: Some("opus".into()),
            effort: Some(EffortLevel::Max),
            permission_mode: Some(PermissionMode::Auto),
            allowed_tools: vec!["Bash".into()],
            mcp_config: Some("/tmp/mcp.json".into()),
            bare: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ClaudeCliConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name.as_deref(), Some("my-session"));
        assert_eq!(parsed.model.as_deref(), Some("opus"));
        assert_eq!(parsed.effort, Some(EffortLevel::Max));
        assert_eq!(parsed.permission_mode, Some(PermissionMode::Auto));
        assert_eq!(parsed.allowed_tools, vec!["Bash"]);
        assert_eq!(parsed.mcp_config.as_deref(), Some("/tmp/mcp.json"));
        assert!(parsed.bare);
    }

    #[test]
    fn empty_json_deserializes_to_default() {
        let config: ClaudeCliConfig = serde_json::from_str("{}").unwrap();
        assert!(config.to_args().is_empty());
        assert!(!config.bare);
        assert!(!config.verbose);
        assert!(config.allowed_tools.is_empty());
    }

    #[test]
    fn agent_and_agents_fields() {
        let config = ClaudeCliConfig {
            agent: Some("reviewer".into()),
            agents: Some(r#"{"reviewer":{"description":"Reviews code"}}"#.into()),
            ..Default::default()
        };
        let args = config.to_args();
        assert!(args.contains(&"--agent".to_string()));
        assert!(args.contains(&"reviewer".to_string()));
        assert!(args.contains(&"--agents".to_string()));
    }

    #[test]
    fn preset_round_trips_through_json() {
        let preset = ClaudePreset {
            name: "sandbox".into(),
            description: "Sandboxed agent with no permissions".into(),
            config: ClaudeCliConfig {
                permission_mode: Some(PermissionMode::DontAsk),
                bare: true,
                ..Default::default()
            },
        };
        let json = serde_json::to_string_pretty(&preset).unwrap();
        let parsed: ClaudePreset = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "sandbox");
        assert_eq!(parsed.description, "Sandboxed agent with no permissions");
        assert_eq!(parsed.config.permission_mode, Some(PermissionMode::DontAsk));
        assert!(parsed.config.bare);
    }

    #[test]
    fn preset_flattens_config_fields() {
        let json = r#"{
            "name": "fast",
            "description": "Fast mode",
            "model": "sonnet",
            "effort": "low",
            "bare": true
        }"#;
        let preset: ClaudePreset = serde_json::from_str(json).unwrap();
        assert_eq!(preset.name, "fast");
        assert_eq!(preset.config.model.as_deref(), Some("sonnet"));
        assert_eq!(preset.config.effort, Some(EffortLevel::Low));
        assert!(preset.config.bare);
    }

    #[test]
    fn preset_config_produces_correct_args() {
        let preset = ClaudePreset {
            name: "review".into(),
            description: "Code review agent".into(),
            config: ClaudeCliConfig {
                model: Some("opus".into()),
                permission_mode: Some(PermissionMode::Plan),
                append_system_prompt: Some("You are a code reviewer.".into()),
                ..Default::default()
            },
        };
        let args = preset.config.to_args();
        assert_eq!(
            args,
            vec![
                "--model",
                "opus",
                "--permission-mode",
                "plan",
                "--append-system-prompt",
                "You are a code reviewer."
            ]
        );
    }
}
