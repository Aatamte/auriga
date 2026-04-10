use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SandboxMode {
    ReadOnly,
    WorkspaceWrite,
    DangerFullAccess,
}

impl SandboxMode {
    fn as_cli_str(&self) -> &str {
        match self {
            Self::ReadOnly => "read-only",
            Self::WorkspaceWrite => "workspace-write",
            Self::DangerFullAccess => "danger-full-access",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApprovalPolicy {
    Untrusted,
    OnRequest,
    Never,
}

impl ApprovalPolicy {
    fn as_cli_str(&self) -> &str {
        match self {
            Self::Untrusted => "untrusted",
            Self::OnRequest => "on-request",
            Self::Never => "never",
        }
    }
}

/// Domain model for the OpenAI Codex CLI configuration.
/// Maps to `codex --help` and `codex exec --help` flags.
/// Serializable for persistence in `AgentConfig.provider_config`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexCliConfig {
    // --- Model & profile ---
    pub model: Option<String>,
    pub profile: Option<String>,

    // --- Sandbox & approvals ---
    pub sandbox: Option<SandboxMode>,
    /// Interactive-mode only. `codex exec` rejects `-a`.
    pub approval: Option<ApprovalPolicy>,
    pub full_auto: bool,
    pub dangerously_bypass: bool,

    // --- Working directory ---
    pub cd: Option<String>,
    pub add_dirs: Vec<String>,

    // --- Config overrides (`-c key=value`, repeatable) ---
    pub config_overrides: Vec<(String, String)>,

    // --- Feature flags (`--enable` / `--disable`, repeatable) ---
    pub enable_features: Vec<String>,
    pub disable_features: Vec<String>,

    // --- Provider selection ---
    pub oss: bool,
    pub local_provider: Option<String>,

    // --- Images ---
    pub images: Vec<String>,

    // --- Interactive-only behavior ---
    pub search: bool,
    pub no_alt_screen: bool,

    // --- Exec-only flags ---
    pub ephemeral: bool,
    pub skip_git_repo_check: bool,
    pub output_schema: Option<String>,

    // --- Environment variables passed to the CLI process ---
    pub env: Vec<(String, String)>,
}

impl CodexCliConfig {
    /// Args shared between interactive (`codex`) and exec (`codex exec`) invocations.
    fn push_shared_args(&self, args: &mut Vec<String>) {
        for (k, v) in &self.config_overrides {
            args.extend(["-c".into(), format!("{}={}", k, v)]);
        }
        for feat in &self.enable_features {
            args.extend(["--enable".into(), feat.clone()]);
        }
        for feat in &self.disable_features {
            args.extend(["--disable".into(), feat.clone()]);
        }
        if let Some(ref model) = self.model {
            args.extend(["--model".into(), model.clone()]);
        }
        if let Some(ref profile) = self.profile {
            args.extend(["--profile".into(), profile.clone()]);
        }
        if let Some(ref sandbox) = self.sandbox {
            args.extend(["--sandbox".into(), sandbox.as_cli_str().into()]);
        }
        if self.full_auto {
            args.push("--full-auto".into());
        }
        if self.dangerously_bypass {
            args.push("--dangerously-bypass-approvals-and-sandbox".into());
        }
        if let Some(ref cd) = self.cd {
            args.extend(["--cd".into(), cd.clone()]);
        }
        for dir in &self.add_dirs {
            args.extend(["--add-dir".into(), dir.clone()]);
        }
        if self.oss {
            args.push("--oss".into());
        }
        if let Some(ref lp) = self.local_provider {
            args.extend(["--local-provider".into(), lp.clone()]);
        }
        for img in &self.images {
            args.extend(["--image".into(), img.clone()]);
        }
    }

    /// Build argv for launching interactive `codex`.
    pub fn to_interactive_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        self.push_shared_args(&mut args);
        if let Some(ref approval) = self.approval {
            args.extend(["--ask-for-approval".into(), approval.as_cli_str().into()]);
        }
        if self.search {
            args.push("--search".into());
        }
        if self.no_alt_screen {
            args.push("--no-alt-screen".into());
        }
        args
    }

    /// Build argv suffix for `codex exec` (caller prepends the `exec` subcommand
    /// and any positional prompt / stdin marker). Always includes `--json`.
    pub fn to_exec_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        self.push_shared_args(&mut args);
        args.push("--json".into());
        if self.ephemeral {
            args.push("--ephemeral".into());
        }
        if self.skip_git_repo_check {
            args.push("--skip-git-repo-check".into());
        }
        if let Some(ref schema) = self.output_schema {
            args.extend(["--output-schema".into(), schema.clone()]);
        }
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_interactive_is_empty() {
        let c = CodexCliConfig::default();
        assert!(c.to_interactive_args().is_empty());
    }

    #[test]
    fn default_exec_is_just_json() {
        let c = CodexCliConfig::default();
        assert_eq!(c.to_exec_args(), vec!["--json"]);
    }

    #[test]
    fn model_and_profile() {
        let c = CodexCliConfig {
            model: Some("gpt-5".into()),
            profile: Some("review".into()),
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--model", "gpt-5", "--profile", "review"]
        );
    }

    #[test]
    fn sandbox_values_serialize() {
        for (mode, expected) in [
            (SandboxMode::ReadOnly, "read-only"),
            (SandboxMode::WorkspaceWrite, "workspace-write"),
            (SandboxMode::DangerFullAccess, "danger-full-access"),
        ] {
            let c = CodexCliConfig {
                sandbox: Some(mode),
                ..Default::default()
            };
            assert_eq!(c.to_interactive_args(), vec!["--sandbox", expected]);
        }
    }

    #[test]
    fn approval_is_interactive_only() {
        let c = CodexCliConfig {
            approval: Some(ApprovalPolicy::OnRequest),
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--ask-for-approval", "on-request"]
        );
        // Exec must NOT include --ask-for-approval
        let exec = c.to_exec_args();
        assert!(!exec.iter().any(|a| a == "--ask-for-approval"));
    }

    #[test]
    fn search_and_no_alt_screen_are_interactive_only() {
        let c = CodexCliConfig {
            search: true,
            no_alt_screen: true,
            ..Default::default()
        };
        let i = c.to_interactive_args();
        assert!(i.contains(&"--search".to_string()));
        assert!(i.contains(&"--no-alt-screen".to_string()));
        let e = c.to_exec_args();
        assert!(!e.contains(&"--search".to_string()));
        assert!(!e.contains(&"--no-alt-screen".to_string()));
    }

    #[test]
    fn ephemeral_and_skip_git_are_exec_only() {
        let c = CodexCliConfig {
            ephemeral: true,
            skip_git_repo_check: true,
            ..Default::default()
        };
        let i = c.to_interactive_args();
        assert!(!i.contains(&"--ephemeral".to_string()));
        assert!(!i.contains(&"--skip-git-repo-check".to_string()));
        let e = c.to_exec_args();
        assert!(e.contains(&"--ephemeral".to_string()));
        assert!(e.contains(&"--skip-git-repo-check".to_string()));
    }

    #[test]
    fn config_overrides_emit_minus_c() {
        let c = CodexCliConfig {
            config_overrides: vec![
                ("model".into(), "\"o3\"".into()),
                (
                    "sandbox_permissions".into(),
                    "[\"disk-full-read-access\"]".into(),
                ),
            ],
            ..Default::default()
        };
        let args = c.to_interactive_args();
        assert_eq!(
            args,
            vec![
                "-c",
                "model=\"o3\"",
                "-c",
                "sandbox_permissions=[\"disk-full-read-access\"]",
            ]
        );
    }

    #[test]
    fn enable_and_disable_features() {
        let c = CodexCliConfig {
            enable_features: vec!["web-search".into()],
            disable_features: vec!["telemetry".into()],
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--enable", "web-search", "--disable", "telemetry"]
        );
    }

    #[test]
    fn add_dirs_and_cd() {
        let c = CodexCliConfig {
            cd: Some("/tmp/work".into()),
            add_dirs: vec!["/a".into(), "/b".into()],
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--cd", "/tmp/work", "--add-dir", "/a", "--add-dir", "/b"]
        );
    }

    #[test]
    fn images() {
        let c = CodexCliConfig {
            images: vec!["/p1.png".into(), "/p2.png".into()],
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--image", "/p1.png", "--image", "/p2.png"]
        );
    }

    #[test]
    fn oss_and_local_provider() {
        let c = CodexCliConfig {
            oss: true,
            local_provider: Some("ollama".into()),
            ..Default::default()
        };
        assert_eq!(
            c.to_interactive_args(),
            vec!["--oss", "--local-provider", "ollama"]
        );
    }

    #[test]
    fn full_auto_and_bypass_flags() {
        let c = CodexCliConfig {
            full_auto: true,
            dangerously_bypass: true,
            ..Default::default()
        };
        let args = c.to_interactive_args();
        assert!(args.contains(&"--full-auto".to_string()));
        assert!(args.contains(&"--dangerously-bypass-approvals-and-sandbox".to_string()));
    }

    #[test]
    fn output_schema_is_exec_only() {
        let c = CodexCliConfig {
            output_schema: Some("/tmp/schema.json".into()),
            ..Default::default()
        };
        assert!(c.to_interactive_args().is_empty());
        let e = c.to_exec_args();
        assert!(e.contains(&"--output-schema".to_string()));
        assert!(e.contains(&"/tmp/schema.json".to_string()));
    }

    #[test]
    fn round_trips_through_json() {
        let c = CodexCliConfig {
            model: Some("gpt-5".into()),
            sandbox: Some(SandboxMode::WorkspaceWrite),
            approval: Some(ApprovalPolicy::OnRequest),
            full_auto: true,
            add_dirs: vec!["/extra".into()],
            config_overrides: vec![("foo.bar".into(), "1".into())],
            ephemeral: true,
            ..Default::default()
        };
        let json = serde_json::to_string(&c).unwrap();
        let parsed: CodexCliConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model.as_deref(), Some("gpt-5"));
        assert_eq!(parsed.sandbox, Some(SandboxMode::WorkspaceWrite));
        assert_eq!(parsed.approval, Some(ApprovalPolicy::OnRequest));
        assert!(parsed.full_auto);
        assert_eq!(parsed.add_dirs, vec!["/extra"]);
        assert_eq!(
            parsed.config_overrides,
            vec![("foo.bar".into(), "1".into())]
        );
        assert!(parsed.ephemeral);
    }

    #[test]
    fn empty_json_is_default() {
        let c: CodexCliConfig = serde_json::from_str("{}").unwrap();
        assert!(c.to_interactive_args().is_empty());
        assert_eq!(c.to_exec_args(), vec!["--json"]);
    }
}
