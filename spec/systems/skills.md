# Skills — Technical Specification

## Skill Trait

```rust
pub trait Skill: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn trigger(&self) -> SkillTrigger;
    fn execute(&self, ctx: &SkillContext) -> anyhow::Result<SkillResult>;
}
```

## Types

```rust
pub struct SkillId(pub Uuid);

pub enum SkillTrigger {
    OnDemand,
    OnSessionStart,
    OnSessionEnd,
}

impl SkillTrigger {
    pub fn is_on_demand(&self) -> bool;
    pub fn runs_on_session_start(&self) -> bool;
    pub fn runs_on_session_end(&self) -> bool;
}

pub struct SkillContext {
    pub agent_id: AgentId,
    pub arguments: serde_json::Value,
}

pub struct SkillResult {
    pub id: SkillId,
    pub skill_name: String,
    pub agent_id: AgentId,
    pub timestamp: String,
    pub success: bool,
    pub payload: serde_json::Value,
}

pub struct SkillStatus {
    pub name: String,
    pub description: String,
    pub trigger: SkillTrigger,
    pub enabled: bool,
}
```

## Registry

```rust
struct SkillEntry {
    skill: Box<dyn Skill>,
    enabled: bool,
}

pub struct SkillRegistry {
    entries: Vec<SkillEntry>,
}
```

Methods:
```
register(skill)                                    — panics on duplicate name
count() → usize
names() → Vec<&str>
set_enabled(name, bool) → bool                    — returns false if not found
is_enabled(name) → bool
skills_info() → Vec<SkillStatus>
execute(name, ctx) → Result<SkillResult>           — errors if not found or disabled
run_session_start(ctx) → Vec<Result<SkillResult>>  — runs matching enabled skills
run_session_end(ctx) → Vec<Result<SkillResult>>    — runs matching enabled skills
```

Dispatch logic: iterate entries, filter by `enabled && trigger matches`, call `execute()`, collect results. Unlike classifiers, skill execution is fallible — each result is wrapped in `Result`.
