use crate::Skill;

/// Built-in skill: review recent changes before declaring work done.
///
/// When an agent downloads this skill into a project, its `SKILL.md`
/// lives at `.claude/skills/code-review/SKILL.md` (and the mirror
/// under `.agents/`). The agent's own skill system triggers the
/// body below.
pub struct CodeReviewSkill;

const BODY: &str = "\
# Code Review

Use this skill whenever you are about to declare a task done. Review \
every change you made in this session before finishing.

For each changed file, check:

1. **Correctness** — does the logic actually do what was intended? \
   Trace the flow with concrete values.
2. **Edge cases** — inputs or states that would break it. Empty \
   collections, missing optionals, zero, boundary indices.
3. **Error handling** — failures are handled, not silenced. No \
   `unwrap` on user input, no swallowed errors.
4. **Tests** — existing tests still pass. New logic has new tests.
5. **Style** — matches the surrounding code conventions.

List any issues you find. If everything looks good, say so briefly. \
Do not perform the review silently — produce a short written summary \
so the user can see what you checked.
";

impl Skill for CodeReviewSkill {
    fn name(&self) -> &str {
        "code-review"
    }

    fn description(&self) -> &str {
        "Review recent code changes for correctness, edge cases, error \
         handling, tests, and style. Use before declaring a task done."
    }

    fn body(&self) -> &str {
        BODY
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_and_description() {
        assert_eq!(CodeReviewSkill.name(), "code-review");
        assert!(!CodeReviewSkill.description().is_empty());
    }

    #[test]
    fn body_is_non_empty_markdown() {
        let body = CodeReviewSkill.body();
        assert!(body.starts_with("# Code Review"));
        assert!(body.contains("Correctness"));
        assert!(body.contains("Edge cases"));
    }
}
