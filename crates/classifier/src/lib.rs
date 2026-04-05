mod classifier_trait;
pub mod config;
mod registry;
pub mod runtime;
mod schema;
mod types;

pub use classifier_trait::Classifier;
pub use config::{ClassifierConfig, ClassifierType, ConfigClassifier};
pub use registry::ClassifierRegistry;
pub use runtime::{
    ClassifierRuntime, CliRuntime, CliRuntimeConfig, LlmRuntimeStub, RuntimePrediction,
};
pub use schema::CLASSIFICATIONS_TABLE_SQL;
pub use types::{
    ClassificationId, ClassificationResult, ClassifierStatus, ClassifierTrigger, Notification,
    TriggerPhase, TurnFilter,
};
