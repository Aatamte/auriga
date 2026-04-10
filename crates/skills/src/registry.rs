use crate::{Skill, SkillStatus};

/// Holds every skill that ships with the orchestrator binary.
///
/// The registry is purely a lookup: it owns the `Box<dyn Skill>`
/// instances and exposes their metadata + body content. Whether a
/// skill is "downloaded" (i.e. written to disk in the current
/// project) is not tracked here — that is a filesystem concern,
/// resolved by the caller via `skills_storage::is_downloaded`.
pub struct SkillRegistry {
    skills: Vec<Box<dyn Skill>>,
}

impl SkillRegistry {
    pub fn new() -> Self {
        Self { skills: Vec::new() }
    }

    /// Register a skill. Panics on duplicate name — skill names are
    /// compile-time identifiers and a collision is a bug.
    pub fn register(&mut self, skill: Box<dyn Skill>) {
        let name = skill.name().to_string();
        assert!(
            !self.skills.iter().any(|s| s.name() == name),
            "duplicate skill name: '{name}' is already registered",
        );
        self.skills.push(skill);
    }

    pub fn count(&self) -> usize {
        self.skills.len()
    }

    /// Look up a skill by name.
    pub fn get(&self, name: &str) -> Option<&dyn Skill> {
        self.skills
            .iter()
            .find(|s| s.name() == name)
            .map(|s| s.as_ref())
    }

    /// Iterate over all registered skills.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Skill> {
        self.skills.iter().map(|s| s.as_ref())
    }

    /// Produce a display-ready summary of every registered skill.
    ///
    /// `downloaded_names` is the set of skill names whose `SKILL.md`
    /// has been written to disk in the current project (from
    /// `skills_storage::is_downloaded`). The resulting list has one
    /// entry per registered skill, in registration order.
    pub fn skills_info(&self, downloaded: &dyn Fn(&str) -> bool) -> Vec<SkillStatus> {
        self.skills
            .iter()
            .map(|s| SkillStatus {
                name: s.name().to_string(),
                description: s.description().to_string(),
                downloaded: downloaded(s.name()),
            })
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

    struct MockSkill {
        name: &'static str,
        body: &'static str,
    }

    impl Skill for MockSkill {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            "mock skill"
        }
        fn body(&self) -> &str {
            self.body
        }
    }

    fn mock(name: &'static str) -> Box<dyn Skill> {
        Box::new(MockSkill { name, body: "body" })
    }

    #[test]
    fn register_and_count() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a"));
        reg.register(mock("b"));
        assert_eq!(reg.count(), 2);
    }

    #[test]
    #[should_panic(expected = "duplicate skill name")]
    fn duplicate_name_panics() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("dup"));
        reg.register(mock("dup"));
    }

    #[test]
    fn get_by_name() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a"));
        assert!(reg.get("a").is_some());
        assert!(reg.get("nope").is_none());
    }

    #[test]
    fn iter_visits_all() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a"));
        reg.register(mock("b"));
        let names: Vec<&str> = reg.iter().map(|s| s.name()).collect();
        assert_eq!(names, vec!["a", "b"]);
    }

    #[test]
    fn skills_info_marks_downloaded() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a"));
        reg.register(mock("b"));
        let is_downloaded = |name: &str| name == "a";
        let info = reg.skills_info(&is_downloaded);
        assert_eq!(info.len(), 2);
        assert!(info[0].downloaded);
        assert!(!info[1].downloaded);
    }

    #[test]
    fn skills_info_none_downloaded() {
        let mut reg = SkillRegistry::new();
        reg.register(mock("a"));
        let info = reg.skills_info(&|_| false);
        assert!(!info[0].downloaded);
    }
}
