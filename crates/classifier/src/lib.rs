mod classifier_trait;
pub mod config;
mod registry;
pub mod runtime;
mod schema;

pub use classifier_trait::Classifier;
pub use config::{load_config, load_configs, save_config, ConfigClassifier};
pub use registry::ClassifierRegistry;
pub use runtime::{
    ClassifierRuntime, CliRuntime, CliRuntimeConfig, LlmRuntimeStub, RuntimePrediction,
};
pub use schema::CLASSIFICATIONS_TABLE_SQL;

// Re-export types from orchestrator-types for backward compatibility.
pub use orchestrator_types::{
    ClassificationId, ClassificationResult, ClassifierConfig, ClassifierStatus, ClassifierTrigger,
    ClassifierType, ConfigTrigger, LabelConfig, Notification, NotificationConfig, TriggerConfig,
    TriggerPhase, TurnFilter,
};
