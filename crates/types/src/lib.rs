mod agent;
mod agent_config;
mod classifier;
mod classifier_config;
mod claude;
mod claude_settings;
mod codex;
mod command;
mod file_activity;
mod file_tree;
mod focus;
mod message;
mod provider;
mod scrollable;
mod session;
mod skill;
mod tool;
mod trace;
mod turn;

// --- Agent ---
pub use agent::{Agent, AgentId, AgentStatus, DisplayMode};

// --- Turn ---
pub use turn::{
    AssistantMeta, ContentBlock, ImageSource, ImageSourceType, MessageContent, MessageType,
    StopReason, SystemMeta, TokenUsage, ToolResultContent, Turn, TurnBuilder, TurnId, TurnMeta,
    TurnRole, TurnStatus, UserMeta,
};

// --- Trace ---
pub use trace::{Trace, TraceId, TraceStatus};

// --- Focus ---
pub use focus::{FocusState, Page, Panel};

// --- Scrollable ---
pub use scrollable::ScrollDirection;

// --- File activity ---
pub use file_activity::FileActivity;

// --- File tree ---
pub use file_tree::FileEntry;

// --- Classifier ---
pub use classifier::{
    ClassificationId, ClassificationResult, ClassifierStatus, ClassifierTrigger, Notification,
    TriggerPhase, TurnFilter,
};

// --- Classifier config ---
pub use classifier_config::{
    ClassifierConfig, ClassifierType, ConfigTrigger, LabelConfig, NotificationConfig, TriggerConfig,
};

// --- Skill ---
pub use skill::SkillStatus;

// --- Agent config ---
pub use agent_config::{AgentConfig, AgentMode, SystemPromptBuilder};

// --- Command ---
pub use command::CommandSpec;

// --- Message ---
pub use message::{GenerateRequest, GenerateResponse, Message, Role};

// --- Tool ---
pub use tool::{extract_tool_calls, ToolCall, ToolDefinition, ToolOutput};

// --- Session ---
pub use session::{SessionId, SessionStatus};

// --- Provider ---
pub use provider::GenerateError;

// --- Claude CLI ---
pub use claude::{ClaudeCliConfig, EffortLevel, OutputFormat, PermissionMode};
pub use claude_settings::{AttributionConfig, ClaudeSettings, PermissionsConfig, WorktreeConfig};
pub use codex::{ApprovalPolicy, CodexCliConfig, SandboxMode};
