mod agent;
mod file_activity;
mod file_tree;
mod scrollable;
mod trace;
mod turn;

// Re-export all types from the types crate for backward compatibility.
// Downstream crates can keep using `auriga_core::AgentId` etc.
pub use auriga_types::*;

// State containers (stores) — these stay in core.
pub use agent::AgentStore;
pub use file_activity::FileActivityStore;
pub use file_tree::FileTree;
pub use scrollable::Scrollable;
pub use trace::TraceStore;
pub use turn::TurnStore;
