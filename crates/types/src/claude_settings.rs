use serde::{Deserialize, Serialize};

/// Domain model for Claude Code settings.json.
/// Passed to the CLI via `--settings <file>`.
/// See https://json.schemastore.org/claude-code-settings.json
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClaudeSettings {
    // --- Permissions ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permissions: Option<PermissionsConfig>,

    // --- Model & reasoning ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_models: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_overrides: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_mode: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_mode_per_session_opt_in: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub always_thinking_enabled: Option<bool>,

    // --- Environment ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<serde_json::Value>,

    // --- Memory & context ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_memory_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claude_md_excludes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_git_instructions: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respect_gitignore: Option<bool>,

    // --- Language & output ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_style: Option<String>,

    // --- Attribution ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attribution: Option<AttributionConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_co_authored_by: Option<bool>,

    // --- Hooks ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hooks: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_all_hooks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_http_hook_urls: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_hook_allowed_env_vars: Option<Vec<String>>,

    // --- Plugins ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_plugins: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plugin_configs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_known_marketplaces: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_marketplaces: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped_plugins: Option<Vec<String>>,

    // --- MCP servers ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_all_project_mcp_servers: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_mcpjson_servers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled_mcpjson_servers: Option<Vec<String>>,

    // --- Sandbox ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<serde_json::Value>,

    // --- UI ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_verbs: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_tips_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spinner_tips_override: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_progress_bar_enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_turn_duration: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefers_reduced_motion: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_line: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_suggestion: Option<serde_json::Value>,

    // --- Session & storage ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cleanup_period_days: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plans_directory: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_updates_channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub feedback_survey_rate: Option<f64>,

    // --- Auth ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key_helper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_credential_export: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aws_auth_refresh: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_login_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub force_login_org_uuid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub otel_headers_helper: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_web_fetch_preflight: Option<bool>,

    // --- Teams ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub teammate_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub company_announcements: Option<Vec<String>>,

    // --- Worktree ---
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree: Option<WorktreeConfig>,
}

/// Permissions configuration for Claude Code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PermissionsConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub allow: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ask: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub additional_directories: Option<Vec<String>>,
}

/// Attribution configuration for git commits and PRs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AttributionConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pr: Option<String>,
}

/// Worktree configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WorktreeConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sparse_paths: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_settings_serializes_to_empty_object() {
        let settings = ClaudeSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        assert_eq!(json, "{}");
    }

    #[test]
    fn permissions_with_skills_round_trips() {
        let settings = ClaudeSettings {
            permissions: Some(PermissionsConfig {
                allow: vec![
                    "Bash".into(),
                    "Read".into(),
                    "Edit".into(),
                    "Skill(commit)".into(),
                    "Skill(simplify)".into(),
                ],
                deny: vec!["Bash(rm -rf *)".into()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let json = serde_json::to_string_pretty(&settings).unwrap();
        let parsed: ClaudeSettings = serde_json::from_str(&json).unwrap();
        let perms = parsed.permissions.unwrap();
        assert_eq!(perms.allow.len(), 5);
        assert!(perms.allow.contains(&"Skill(commit)".to_string()));
        assert_eq!(perms.deny, vec!["Bash(rm -rf *)"]);
    }

    #[test]
    fn full_settings_round_trips() {
        let settings = ClaudeSettings {
            permissions: Some(PermissionsConfig {
                allow: vec!["Bash".into(), "Read".into()],
                default_mode: Some("auto".into()),
                ..Default::default()
            }),
            model: Some("sonnet".into()),
            effort_level: Some("high".into()),
            auto_memory_enabled: Some(true),
            language: Some("english".into()),
            fast_mode: Some(false),
            sandbox: Some(serde_json::json!({"enabled": true})),
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: ClaudeSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.model.as_deref(), Some("sonnet"));
        assert_eq!(parsed.effort_level.as_deref(), Some("high"));
        assert_eq!(parsed.auto_memory_enabled, Some(true));
        assert_eq!(parsed.language.as_deref(), Some("english"));
    }

    #[test]
    fn deserializes_from_real_schema_format() {
        let json = r#"{
            "permissions": {
                "allow": ["Bash(git add:*)"],
                "ask": ["Bash(gh pr create:*)", "Bash(git commit:*)"],
                "deny": ["Read(*.env)", "Bash(rm:*)", "Bash(curl:*)"],
                "defaultMode": "default"
            },
            "model": "claude-opus-4-6",
            "effortLevel": "high",
            "autoMemoryEnabled": true,
            "includeGitInstructions": true,
            "enabledPlugins": {"formatter@anthropic-tools": true}
        }"#;
        let settings: ClaudeSettings = serde_json::from_str(json).unwrap();
        let perms = settings.permissions.unwrap();
        assert_eq!(perms.default_mode.as_deref(), Some("default"));
        assert_eq!(perms.deny.len(), 3);
        assert_eq!(settings.model.as_deref(), Some("claude-opus-4-6"));
        assert_eq!(settings.effort_level.as_deref(), Some("high"));
        assert!(settings.enabled_plugins.is_some());
    }

    #[test]
    fn skip_serializing_none_fields() {
        let settings = ClaudeSettings {
            model: Some("sonnet".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).unwrap();
        assert!(json.contains("model"));
        assert!(!json.contains("permissions"));
        assert!(!json.contains("hooks"));
        assert!(!json.contains("sandbox"));
    }
}
