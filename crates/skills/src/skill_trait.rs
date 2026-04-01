use crate::{SkillContext, SkillResult, SkillTrigger};

/// Trait that skill implementations must satisfy.
///
/// Each skill has a unique name, a description, a trigger configuration,
/// and an `execute` method that performs the skill's action.
pub trait Skill: Send + Sync {
    /// Unique identifier for this skill (e.g. "file-search").
    fn name(&self) -> &str;

    /// Human-readable description of what this skill does.
    fn description(&self) -> &str;

    /// When this skill can be triggered.
    fn trigger(&self) -> SkillTrigger;

    /// Execute the skill with the given context.
    fn execute(&self, ctx: &SkillContext) -> anyhow::Result<SkillResult>;
}
