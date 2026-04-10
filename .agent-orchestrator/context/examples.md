---
name: examples
description: Canonical implementation examples. How to add pages, widgets, providers, skills, classifiers.
last_updated: 2026-04-06
---

# Canonical Examples

When implementing something new, find the closest existing pattern and follow it.

## Adding a new Page

1. Add variant to `Page` enum in `crates/core/src/focus.rs`
2. Add to `Page::LEFT` or `Page::RIGHT` array and `Page::ALL`
3. Add `label()` match arm
4. Add `WidgetId` variant in `crates/grid/src/layout.rs`
5. Create page widget in `crates/widgets/src/` — follow `classifiers_page.rs` structure
6. Wire into `crates/widgets/src/lib.rs`: mod, pub use, WidgetRegistry field, get_mut match arm
7. Add page rendering in `crates/app/src/main.rs` page match
8. Add key handling in `crates/app/src/input.rs` page match
9. Reference: `classifiers_page.rs` is the canonical list page

## Adding a new WidgetAction

1. Add variant to `WidgetAction` in `crates/widgets/src/lib.rs`
2. Add handler in `App::handle_widget_action()` in `crates/app/src/app.rs`
3. Return the action from the widget that triggers it
4. Reference: `ToggleClassifier` flow is the canonical toggle pattern

## Adding a new Provider

1. Create file in `crates/agent/src/providers/`
2. Implement `Provider` trait: `name()`, `generate()`, `build_command()`
3. Add match arm in `resolve_provider()` in `crates/app/src/app.rs`
4. Reference: `claude.rs` is the only current implementation

## Adding a new Skill

1. Create file in `crates/skills/src/`
2. Implement `Skill` trait: `name()`, `description()`, `trigger()`, `execute()`
3. Register in `App::register_default_skills()` in `crates/app/src/app.rs`
4. Reference: `code_review.rs` is the canonical skill

## Adding a new Classifier

1. Create JSON config in `.agent-orchestrator/classifiers/`
2. Follow schema in `agent-health.json`
3. Loaded automatically by `App::load_classifier_configs()`
