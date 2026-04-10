// Storage layer for agent skills. Callers pass registered `Skill`
// instances in and this module writes their `SKILL.md` file out.
#![allow(dead_code)]

//! Filesystem storage for agent skills.
//!
//! A skill is considered "downloaded" when its `SKILL.md` exists in
//! both of these locations under the current project root:
//!
//! - `.claude/skills/<name>/SKILL.md` — discovered by Claude Code
//! - `.agents/skills/<name>/SKILL.md` — discovered by Codex
//!
//! Both files hold identical content so the same skill is usable by
//! any agent spawned in the project. The format is Anthropic's native
//! `SKILL.md`: YAML frontmatter with `name` and `description`,
//! followed by a markdown body.
//!
//! This module mirrors `config.rs` and `context.rs`: pure functions,
//! synchronous `std::fs`, no background thread. Skills are small,
//! rare writes — no need for the queued storage thread used by SQLite.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Result};
use orchestrator_skills::Skill;

const CLAUDE_SKILLS_DIR: &str = ".claude/skills";
const AGENTS_SKILLS_DIR: &str = ".agents/skills";
const SKILL_FILE: &str = "SKILL.md";

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Paths written by [`write_skill`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrittenPaths {
    pub claude: PathBuf,
    pub agents: PathBuf,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return the `.claude/skills/<name>/SKILL.md` and
/// `.agents/skills/<name>/SKILL.md` paths for this skill under the
/// given root. Pure path computation — does not touch the filesystem.
pub fn skill_paths(root: &Path, name: &str) -> WrittenPaths {
    WrittenPaths {
        claude: root.join(CLAUDE_SKILLS_DIR).join(name).join(SKILL_FILE),
        agents: root.join(AGENTS_SKILLS_DIR).join(name).join(SKILL_FILE),
    }
}

/// A skill counts as "downloaded" when both SKILL.md files exist.
/// If only one is present, it is not considered downloaded — the
/// pair is always written and deleted together.
pub fn is_downloaded(root: &Path, name: &str) -> bool {
    let paths = skill_paths(root, name);
    paths.claude.exists() && paths.agents.exists()
}

/// Write a registered skill to both `.claude/skills/` and
/// `.agents/skills/` under the given project root. Refuses to
/// overwrite an existing skill — delete it first with [`delete_skill`]
/// if you want to replace it.
pub fn write_skill(root: &Path, skill: &dyn Skill) -> Result<WrittenPaths> {
    validate_name(skill.name())?;

    let paths = skill_paths(root, skill.name());
    if paths.claude.exists() {
        bail!("skill already exists at {}", paths.claude.display());
    }
    if paths.agents.exists() {
        bail!("skill already exists at {}", paths.agents.display());
    }

    let rendered = render_skill_md(skill.name(), skill.description(), skill.body());
    write_file(&paths.claude, &rendered)?;
    write_file(&paths.agents, &rendered)?;

    Ok(paths)
}

/// Remove a skill from both `.claude/skills/` and `.agents/skills/`.
/// Removes the containing `<name>/` directory at each location. Errors
/// if neither copy exists — but succeeds if only one is present
/// (partial state is still cleanup-able).
pub fn delete_skill(root: &Path, name: &str) -> Result<()> {
    validate_name(name)?;

    let claude_dir = root.join(CLAUDE_SKILLS_DIR).join(name);
    let agents_dir = root.join(AGENTS_SKILLS_DIR).join(name);

    let claude_exists = claude_dir.exists();
    let agents_exists = agents_dir.exists();

    if !claude_exists && !agents_exists {
        bail!("skill '{name}' is not downloaded");
    }

    if claude_exists {
        fs::remove_dir_all(&claude_dir)?;
    }
    if agents_exists {
        fs::remove_dir_all(&agents_dir)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Internal: validation + rendering
// ---------------------------------------------------------------------------

/// Skill names must be kebab-case: lowercase letters, digits, and
/// hyphens, starting with a lowercase letter. Keeps directory names
/// portable and matches Anthropic's convention for built-in skills.
fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() {
        bail!("skill name is empty");
    }
    let first = name.chars().next().unwrap();
    if !first.is_ascii_lowercase() {
        bail!("skill name must start with a lowercase letter; got '{name}'");
    }
    for c in name.chars() {
        if !(c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
            bail!("skill name must be kebab-case (a-z, 0-9, -); got '{name}'");
        }
    }
    Ok(())
}

fn render_skill_md(name: &str, description: &str, body: &str) -> String {
    // YAML frontmatter with the two required fields. Escape embedded
    // double quotes so the frontmatter stays valid for strict parsers.
    let escaped = description.replace('"', "\\\"");
    let body_trimmed = body.trim_end();
    format!("---\nname: {name}\ndescription: \"{escaped}\"\n---\n\n{body_trimmed}\n")
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    struct TestSkill {
        name: &'static str,
        description: &'static str,
        body: &'static str,
    }

    impl Skill for TestSkill {
        fn name(&self) -> &str {
            self.name
        }
        fn description(&self) -> &str {
            self.description
        }
        fn body(&self) -> &str {
            self.body
        }
    }

    fn sample() -> TestSkill {
        TestSkill {
            name: "commit-message",
            description: "write a conventional commit from staged changes",
            body: "# Commit Message\n\nWrite a conventional commit.",
        }
    }

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "skills-storage-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn skill_paths_point_to_both_locations() {
        let root = PathBuf::from("/tmp/project");
        let paths = skill_paths(&root, "foo");
        assert_eq!(
            paths.claude,
            PathBuf::from("/tmp/project/.claude/skills/foo/SKILL.md")
        );
        assert_eq!(
            paths.agents,
            PathBuf::from("/tmp/project/.agents/skills/foo/SKILL.md")
        );
    }

    #[test]
    fn write_skill_creates_both_files_with_identical_content() {
        let root = tempdir();
        let paths = write_skill(&root, &sample()).unwrap();

        assert!(paths.claude.exists());
        assert!(paths.agents.exists());
        let claude = fs::read_to_string(&paths.claude).unwrap();
        let agents = fs::read_to_string(&paths.agents).unwrap();
        assert_eq!(claude, agents);

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn written_skill_has_frontmatter_and_body() {
        let root = tempdir();
        let paths = write_skill(&root, &sample()).unwrap();
        let contents = fs::read_to_string(&paths.claude).unwrap();

        assert!(contents.starts_with("---\n"));
        assert!(contents.contains("name: commit-message"));
        assert!(
            contents.contains("description: \"write a conventional commit from staged changes\"")
        );
        assert!(contents.contains("# Commit Message"));
        assert!(contents.trim_end().ends_with("Write a conventional commit."));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn is_downloaded_false_when_missing() {
        let root = tempdir();
        assert!(!is_downloaded(&root, "commit-message"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn is_downloaded_true_after_write() {
        let root = tempdir();
        write_skill(&root, &sample()).unwrap();
        assert!(is_downloaded(&root, "commit-message"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn is_downloaded_false_when_only_one_copy_exists() {
        let root = tempdir();
        let paths = write_skill(&root, &sample()).unwrap();
        // Nuke only the agents copy — partial state is not "downloaded".
        fs::remove_dir_all(paths.agents.parent().unwrap()).unwrap();
        assert!(!is_downloaded(&root, "commit-message"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_skill_refuses_to_overwrite_existing() {
        let root = tempdir();
        write_skill(&root, &sample()).unwrap();

        let err = write_skill(&root, &sample()).unwrap_err();
        assert!(err.to_string().contains("already exists"));

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_skill_removes_both_dirs() {
        let root = tempdir();
        write_skill(&root, &sample()).unwrap();
        assert!(is_downloaded(&root, "commit-message"));

        delete_skill(&root, "commit-message").unwrap();
        assert!(!is_downloaded(&root, "commit-message"));
        assert!(!root.join(".claude/skills/commit-message").exists());
        assert!(!root.join(".agents/skills/commit-message").exists());

        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_skill_errors_when_not_downloaded() {
        let root = tempdir();
        let err = delete_skill(&root, "nope").unwrap_err();
        assert!(err.to_string().contains("not downloaded"));
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn delete_skill_cleans_partial_state() {
        let root = tempdir();
        let paths = write_skill(&root, &sample()).unwrap();
        // Remove only the claude copy — delete_skill should still
        // succeed and clean up the remaining agents copy.
        fs::remove_dir_all(paths.claude.parent().unwrap()).unwrap();
        delete_skill(&root, "commit-message").unwrap();
        assert!(!root.join(".agents/skills/commit-message").exists());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn write_skill_rejects_non_kebab_case_name() {
        struct Bad;
        impl Skill for Bad {
            fn name(&self) -> &str {
                "BadName"
            }
            fn description(&self) -> &str {
                "d"
            }
            fn body(&self) -> &str {
                "b"
            }
        }
        let root = tempdir();
        assert!(write_skill(&root, &Bad).is_err());
        fs::remove_dir_all(&root).ok();
    }

    #[test]
    fn render_escapes_quotes_in_description() {
        let md = render_skill_md("x", "has \"quotes\" in it", "body");
        assert!(md.contains("description: \"has \\\"quotes\\\" in it\""));
    }

    #[test]
    fn validate_name_accepts_kebab_case() {
        assert!(validate_name("foo-bar-1").is_ok());
        assert!(validate_name("a").is_ok());
    }

    #[test]
    fn validate_name_rejects_uppercase() {
        assert!(validate_name("Foo").is_err());
    }

    #[test]
    fn validate_name_rejects_underscore() {
        assert!(validate_name("foo_bar").is_err());
    }

    #[test]
    fn validate_name_rejects_leading_digit() {
        assert!(validate_name("1foo").is_err());
    }

    #[test]
    fn validate_name_rejects_empty() {
        assert!(validate_name("").is_err());
    }
}
