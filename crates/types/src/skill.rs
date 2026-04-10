/// Summary of a registered skill for UI display.
///
/// This is the display-side view of a skill: the in-memory registry
/// knows name/description/body from the trait, and storage layers
/// provide the `downloaded` flag that tells the prompts page whether
/// the skill's `SKILL.md` has been written to `.claude/skills/` and
/// `.agents/skills/` in the current project.
#[derive(Debug, Clone)]
pub struct SkillStatus {
    pub name: String,
    pub description: String,
    pub downloaded: bool,
}
