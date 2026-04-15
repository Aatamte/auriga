//! Integration tests for auriga-skills public API

use auriga_skills::{CodeReviewSkill, Skill, SkillRegistry, SkillStatus};

// -- SkillRegistry tests --

#[test]
fn skill_registry_new_is_empty() {
    let registry = SkillRegistry::new();
    assert_eq!(registry.count(), 0);
}

#[test]
fn skill_registry_register_and_count() {
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(CodeReviewSkill));
    assert_eq!(registry.count(), 1);
}

#[test]
fn skill_registry_get_by_name() {
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(CodeReviewSkill));

    assert!(registry.get("code-review").is_some());
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn skill_registry_iter() {
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(CodeReviewSkill));

    let names: Vec<&str> = registry.iter().map(|s| s.name()).collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "code-review");
}

#[test]
fn skill_registry_skills_info() {
    let mut registry = SkillRegistry::new();
    registry.register(Box::new(CodeReviewSkill));

    // All not downloaded
    let info = registry.skills_info(&|_| false);
    assert_eq!(info.len(), 1);
    assert!(!info[0].downloaded);

    // All downloaded
    let info = registry.skills_info(&|_| true);
    assert!(info[0].downloaded);
}

// -- CodeReviewSkill tests --

#[test]
fn code_review_skill_has_name() {
    assert_eq!(CodeReviewSkill.name(), "code-review");
}

#[test]
fn code_review_skill_has_description() {
    assert!(!CodeReviewSkill.description().is_empty());
}

#[test]
fn code_review_skill_has_body() {
    assert!(!CodeReviewSkill.body().is_empty());
}

// -- Skill trait tests --

#[test]
fn skill_trait_methods() {
    // All trait methods should return non-empty strings
    assert!(!CodeReviewSkill.name().is_empty());
    assert!(!CodeReviewSkill.description().is_empty());
    assert!(!CodeReviewSkill.body().is_empty());
}

// -- SkillStatus tests --

#[test]
fn skill_status_structure() {
    let status = SkillStatus {
        name: "test-skill".to_string(),
        description: "A test skill".to_string(),
        downloaded: true,
    };

    assert_eq!(status.name, "test-skill");
    assert_eq!(status.description, "A test skill");
    assert!(status.downloaded);
}

#[test]
fn skill_status_not_downloaded() {
    let status = SkillStatus {
        name: "test".to_string(),
        description: "desc".to_string(),
        downloaded: false,
    };

    assert!(!status.downloaded);
}
