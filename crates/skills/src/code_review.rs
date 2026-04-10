use crate::{Skill, SkillContext, SkillId, SkillResult, SkillTrigger};

/// Built-in skill that reviews recent changes for correctness.
///
/// When invoked, it produces a structured review prompt that the agent
/// can use to audit its own recent edits — checking for bugs, missed
/// edge cases, and style violations.
pub struct CodeReviewSkill;

impl Skill for CodeReviewSkill {
    fn name(&self) -> &str {
        "code-review"
    }

    fn description(&self) -> &str {
        "Review recent code changes for correctness, bugs, and style"
    }

    fn trigger(&self) -> SkillTrigger {
        SkillTrigger::OnDemand
    }

    fn execute(&self, ctx: &SkillContext) -> anyhow::Result<SkillResult> {
        let instructions = concat!(
            "Review the changes you have made in this session.\n\n",
            "For each changed file, check:\n",
            "1. Correctness — does the logic do what was intended?\n",
            "2. Edge cases — are there inputs or states that would break it?\n",
            "3. Error handling — are failures handled, not silenced?\n",
            "4. Tests — do existing tests still pass? Are new tests needed?\n",
            "5. Style — does it match the surrounding code conventions?\n\n",
            "List any issues found. If everything looks good, say so briefly.",
        );

        Ok(SkillResult {
            id: SkillId::new(),
            skill_name: self.name().into(),
            agent_id: ctx.agent_id,
            timestamp: chrono_now(),
            success: true,
            payload: serde_json::json!({ "instructions": instructions }),
        })
    }
}

fn chrono_now() -> String {
    // Simple ISO-ish timestamp without pulling in chrono
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}s", dur.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use orchestrator_core::AgentId;

    fn test_ctx() -> SkillContext {
        SkillContext {
            agent_id: AgentId::from_u128(1),
            arguments: serde_json::Value::Null,
        }
    }

    #[test]
    fn name_and_description() {
        let skill = CodeReviewSkill;
        assert_eq!(skill.name(), "code-review");
        assert!(!skill.description().is_empty());
    }

    #[test]
    fn trigger_is_on_demand() {
        let skill = CodeReviewSkill;
        assert_eq!(skill.trigger(), SkillTrigger::OnDemand);
    }

    #[test]
    fn execute_returns_instructions() {
        let skill = CodeReviewSkill;
        let result = skill.execute(&test_ctx()).unwrap();
        assert_eq!(result.skill_name, "code-review");
        assert!(result.success);
        assert!(result.payload["instructions"]
            .as_str()
            .unwrap()
            .contains("Review the changes"));
    }
}
