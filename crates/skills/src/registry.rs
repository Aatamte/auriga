use crate::{Skill, SkillContext, SkillResult, SkillStatus};

struct SkillEntry {
    skill: Box<dyn Skill>,
    enabled: bool,
}

/// Holds registered skills and dispatches execution requests.
pub struct SkillRegistry {
    entries: Vec<SkillEntry>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a skill (enabled by default). Panics on duplicate name.
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.name().to_string();
        assert!(
            !self.entries.iter().any(|e| e.skill.name() == name),
            "duplicate skill name: '{name}' is already registered",
        );
        self.entries.push(SkillEntry {
            skill,
            enabled: true,
        });
    }

    pub fn count(&self) -> usize {
        self.entries.len()
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries.iter().map(|e| e.skill.name()).collect()
    }

    /// Enable or disable a skill by name. Returns false if not found.
    pub fn set_enabled(&mut self, name: &str, enabled: bool) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.skill.name() == name) {
            entry.enabled = enabled;
            true
        } else {
            false
        }
    }

    pub fn is_enabled(&self, name: &str) -> bool {
        self.entries
            .iter()
            .find(|e| e.skill.name() == name)
            .is_some_and(|e| e.enabled)
    }

    /// Summary info for all registered skills.
    pub fn skills_info(&self) -> Vec<SkillStatus> {
        self.entries
            .iter()
            .map(|e| SkillStatus {
                name: e.skill.name().to_string(),
                description: e.skill.description().to_string(),
                trigger: e.skill.trigger(),
                enabled: e.enabled,
            })
            .collect()
    }

    /// Execute a skill by name. Returns error if not found or disabled.
    pub fn execute(&self, name: &str, ctx: &SkillContext) -> anyhow::Result<SkillResult> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.skill.name() == name)
            .ok_or_else(|| anyhow::anyhow!("unknown skill: '{name}'"))?;

        if !entry.enabled {
            anyhow::bail!("skill '{name}' is disabled");
        }

        entry.skill.execute(ctx)
    }

    /// Run all enabled skills triggered on session start.
    pub fn run_session_start(&self, ctx: &SkillContext) -> Vec<anyhow::Result<SkillResult>> {
        self.entries
            .iter()
            .filter(|e| e.enabled && e.skill.trigger().runs_on_session_start())
            .map(|e| e.skill.execute(ctx))
            .collect()
    }

    /// Run all enabled skills triggered on session end.
    pub fn run_session_end(&self, ctx: &SkillContext) -> Vec<anyhow::Result<SkillResult>> {
        self.entries
            .iter()
            .filter(|e| e.enabled && e.skill.trigger().runs_on_session_end())
            .map(|e| e.skill.execute(ctx))
            .collect()
    }
}

impl Default for SkillRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{SkillId, SkillTrigger};
    use orchestrator_core::AgentId;

    struct MockSkill {
        name: &'static str,
        trigger: SkillTrigger,
    }

    impl Skill for MockSkill {
        fn name(&self) -> &str {
            self.name
        }

        fn description(&self) -> &str {
            "mock skill"
        }

        fn trigger(&self) -> SkillTrigger {
            self.trigger
        }

        fn execute(&self, ctx: &SkillContext) -> anyhow::Result<SkillResult> {
            Ok(SkillResult {
                id: SkillId::new(),
                skill_name: self.name.into(),
                agent_id: ctx.agent_id,
                timestamp: "2026-01-01T00:00:00Z".into(),
                success: true,
                payload: serde_json::json!({"ran": true}),
            })
        }
    }

    fn mock(name: &'static str, trigger: SkillTrigger) -> Box<dyn Skill> {
        Box::new(MockSkill { name, trigger })
    }

    fn test_ctx() -> SkillContext {
        SkillContext {
            agent_id: AgentId::from_u128(1),
            arguments: serde_json::Value::Null,
        }
    }

    #[test]
    fn register_and_count() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        reg.register(mock("b", SkillTrigger::OnSessionStart));
        assert_eq!(reg.count(), 2);
        assert_eq!(reg.names(), vec!["a", "b"]);
    }

    #[test]
    #[should_panic(expected = "duplicate skill name")]
    fn duplicate_name_panics() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("dup", SkillTrigger::OnDemand));
        reg.register(mock("dup", SkillTrigger::OnSessionEnd));
    }

    #[test]
    fn execute_by_name() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        let result = reg.execute("a", &test_ctx()).unwrap();
        assert_eq!(result.skill_name, "a");
        assert!(result.success);
    }

    #[test]
    fn execute_unknown_errors() {
        let reg = SkillRegistry::new();
        assert!(reg.execute("nope", &test_ctx()).is_err());
    }

    #[test]
    fn disabled_skill_errors() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        reg.set_enabled("a", false);
        assert!(reg.execute("a", &test_ctx()).is_err());
    }

    #[test]
    fn re_enabled_skill_runs() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        reg.set_enabled("a", false);
        reg.set_enabled("a", true);
        assert!(reg.execute("a", &test_ctx()).is_ok());
    }

    #[test]
    fn set_enabled_unknown_returns_false() {
        let mut reg = SkillRegistry::new();
        assert!(!reg.set_enabled("nope", false));
    }

    #[test]
    fn is_enabled_default_true() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        assert!(reg.is_enabled("a"));
    }

    #[test]
    fn run_session_start_fires_matching() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("start", SkillTrigger::OnSessionStart));
        reg.register(mock("end", SkillTrigger::OnSessionEnd));
        reg.register(mock("demand", SkillTrigger::OnDemand));

        let results = reg.run_session_start(&test_ctx());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().skill_name, "start");
    }

    #[test]
    fn run_session_end_fires_matching() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("start", SkillTrigger::OnSessionStart));
        reg.register(mock("end", SkillTrigger::OnSessionEnd));
        reg.register(mock("demand", SkillTrigger::OnDemand));

        let results = reg.run_session_end(&test_ctx());
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].as_ref().unwrap().skill_name, "end");
    }

    #[test]
    fn disabled_skill_skipped_in_session_triggers() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnSessionStart));
        reg.set_enabled("a", false);
        assert!(reg.run_session_start(&test_ctx()).is_empty());
    }

    #[test]
    fn skills_info_returns_all() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a", SkillTrigger::OnDemand));
        reg.register(mock("b", SkillTrigger::OnSessionEnd));
        reg.set_enabled("b", false);

        let info = reg.skills_info();
        assert_eq!(info.len(), 2);
        assert_eq!(info[0].name, "a");
        assert!(info[0].enabled);
        assert_eq!(info[1].name, "b");
        assert!(!info[1].enabled);
    }

    #[test]
    fn empty_registry_session_triggers_empty() {
        let reg = SkillRegistry::new();
        assert!(reg.run_session_start(&test_ctx()).is_empty());
        assert!(reg.run_session_end(&test_ctx()).is_empty());
    }
}
