/// A skill that ships with Auriga binary.
///
/// Each `Skill` is a static piece of content: a name, a one-line
/// description, and a markdown body. The prompts page lists all
/// registered skills; the user can "download" one, which writes its
/// body to `.claude/skills/<name>/SKILL.md` and
/// `.agents/skills/<name>/SKILL.md` in the current project so that
/// Claude Code and Codex agents discover it on spawn.
///
/// The trait is deliberately tiny — skills are content, not
/// behaviour. There is no `execute` step inside Auriga;
/// agents invoke the skill through their own skill systems once the
/// file is on disk.
pub trait Skill: Send + Sync {
    /// Unique kebab-case identifier (e.g. "code-review").
    fn name(&self) -> &str;

    /// One-sentence description of what the skill does, and the
    /// conditions under which an agent should trigger it. This is
    /// the text the model sees when deciding whether to invoke the
    /// skill, so it must be specific.
    fn description(&self) -> &str;

    /// Markdown body written into `SKILL.md` under the YAML
    /// frontmatter. Imperative instructions the agent follows when
    /// the skill is triggered.
    fn body(&self) -> &str;
}
